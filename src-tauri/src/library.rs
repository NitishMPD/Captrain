use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Local, Utc};
use image::{DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use windows::{
    core::PCWSTR,
    Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
};

pub type Result<T> = std::result::Result<T, String>;
fn valid_link_title(value: &str) -> Result<String> {
    let title = value.trim();
    if title.is_empty() || title.chars().count() > 120 || value.chars().any(char::is_control) {
        return Err("Use a link name of 1 to 120 characters without control characters.".into());
    }
    Ok(title.to_string())
}

fn link_filename(title: &str) -> String {
    let name: String = title
        .chars()
        .take(100)
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim_matches([' ', '.']);
    let base = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved = matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        base.strip_prefix(prefix).is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    format!(
        "{}{}.url",
        if reserved { "_" } else { "" },
        if name.is_empty() { "Link" } else { name }
    )
}

pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Region,
    Window,
    Screen,
    Url,
    Video,
}
impl Kind {
    fn extension(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Video => "mp4",
            _ => "png",
        }
    }
    pub fn is_image(self) -> bool {
        matches!(self, Self::Region | Self::Window | Self::Screen)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Window => "window",
            Self::Screen => "screen",
            Self::Url => "url",
            Self::Video => "video",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub created_at: String,
    pub width: u32,
    pub height: u32,
    pub filename: String,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub exported_path: Option<PathBuf>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// Set for a permanent folder: captures filed here are also saved into this PC directory.
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(skip_deserializing, default)]
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupItem {
    pub item: Item,
    pub folder: Option<Folder>,
}

impl BackupItem {
    pub fn validate(&self) -> Result<()> {
        let i = &self.item;
        if Uuid::parse_str(&i.id)
            .map(|id| id.to_string())
            .ok()
            .as_deref()
            != Some(&i.id)
            || chrono::DateTime::parse_from_rfc3339(&i.created_at).is_err()
            || i.filename.is_empty()
            || i.filename.encode_utf16().count() > 255
            || i.filename
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
            || !i.filename.ends_with(&format!(".{}", i.kind.extension()))
            || i.exported_path.is_some()
            || i.title
                .as_deref()
                .is_some_and(|t| valid_link_title(t).is_err())
        {
            return Err("Invalid cloud item metadata.".into());
        }
        if i.kind == Kind::Url {
            crate::links::validate_url(i.url.as_deref().ok_or("Missing cloud URL.")?)?;
        } else if i.url.is_some() || i.width == 0 || i.height == 0 {
            return Err("Invalid cloud media metadata.".into());
        }
        if let Some(folder) = &self.folder {
            if Uuid::parse_str(&folder.id)
                .map(|id| id.to_string())
                .ok()
                .as_deref()
                != Some(&folder.id)
                || folder.path.is_some()
                || i.folder_id.as_deref() != Some(&folder.id)
            {
                return Err("Invalid cloud folder metadata.".into());
            }
            valid_folder_name(&folder.name)?;
        } else if i.folder_id.is_some() {
            return Err("Missing cloud folder metadata.".into());
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize, Default)]
struct Index {
    items: Vec<Item>,
    #[serde(default)]
    folders: Vec<Folder>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredIndex {
    Current(Index),
    Legacy(Vec<Item>),
}
impl StoredIndex {
    fn current(self) -> Index {
        match self {
            Self::Current(index) => index,
            Self::Legacy(items) => Index {
                items,
                folders: vec![],
            },
        }
    }
}
#[derive(Serialize)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: usize,
}

#[derive(Default, Serialize)]
pub struct BatchResult {
    pub completed: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Clone, Copy)]
pub enum Content {
    All,
    Images,
    Captures,
}

pub struct Library {
    pub root: PathBuf,
    items: Vec<Item>,
    folders: Vec<Folder>,
    pub warning: Option<String>,
    writable: bool,
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_file(path, |file| file.write_all(bytes).map_err(err))
}
pub fn atomic_copy(source: &Path, destination: &Path) -> Result<()> {
    atomic_file(destination, |file| {
        std::io::copy(&mut File::open(source).map_err(err)?, file).map_err(err)?;
        Ok(())
    })
}
fn atomic_file(path: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = File::create(&temp).map_err(err)?;
        write(&mut file)?;
        file.sync_all().map_err(err)?;
        drop(file);
        let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
            .map_err(err)
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
use std::os::windows::ffi::OsStrExt;

fn valid_folder_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err("Use a folder name of 1–60 characters.".into());
    }
    Ok(name.to_string())
}

impl Library {
    pub fn backup_items(&self) -> Result<Vec<BackupItem>> {
        if !self.writable {
            return Err("Recover the local index before using cloud backup.".into());
        }
        Ok(self
            .items
            .iter()
            .filter(|i| {
                self.path(i, false).is_file()
                    && (i.kind != Kind::Url
                        || i.url
                            .as_deref()
                            .is_some_and(|u| crate::links::validate_url(u).is_ok()))
            })
            .map(|i| {
                let mut item = i.clone();
                item.exported_path = None;
                let folder = self
                    .folders
                    .iter()
                    .find(|f| Some(&f.id) == item.folder_id.as_ref() && f.path.is_none())
                    .cloned();
                if folder.is_none() {
                    item.folder_id = None;
                }
                BackupItem { item, folder }
            })
            .collect())
    }

    /// Consumes verified files staged on the library volume, outside the library lock.
    pub fn restore_backup(
        &mut self,
        backup: &BackupItem,
        source: &Path,
        thumbnail: Option<&Path>,
    ) -> Result<bool> {
        backup.validate()?;
        if !self.writable {
            return Err("Recover the local index before restoring.".into());
        }
        let item = &backup.item;
        if self.items.iter().any(|i| i.id == item.id) {
            return Ok(false);
        }
        let target = self.path(item, false);
        let thumb = self.path(item, true);
        if target.exists() || (thumbnail.is_some() && thumb.exists()) {
            return Err("An unindexed local file already uses this cloud item's ID. Preserve it before restoring.".into());
        }
        let previous_folders = self.folders.len();
        if let Some(folder) = &backup.folder {
            if !self.folders.iter().any(|f| f.id == folder.id) {
                self.folders.push(folder.clone());
            }
        }
        let result = (|| {
            // Same-volume staging avoids copying large videos while holding the library lock.
            let install = |from: &Path, to: &Path| -> Result<()> {
                let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
                let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
                // Without REPLACE_EXISTING, an external file created concurrently is preserved.
                unsafe {
                    MoveFileExW(
                        PCWSTR(from.as_ptr()),
                        PCWSTR(to.as_ptr()),
                        MOVEFILE_WRITE_THROUGH,
                    )
                    .map_err(err)
                }
            };
            install(source, &target)?;
            if let Some(source) = thumbnail {
                if let Err(error) = install(source, &thumb) {
                    let _ = fs::remove_file(&target);
                    return Err(error);
                }
            }
            self.items.push(item.clone());
            if let Err(error) = self.persist() {
                self.items.pop();
                let _ = fs::remove_file(&target);
                if thumbnail.is_some() {
                    let _ = fs::remove_file(&thumb);
                }
                return Err(error);
            }
            Ok(true)
        })();
        if result.is_err() {
            self.folders.truncate(previous_folders);
        }
        result
    }

    fn persist(&self) -> Result<()> {
        if !self.writable {
            return Err("Capture index needs recovery; existing data was left untouched.".into());
        }
        atomic_write(
            &self.root.join("index.json"),
            &serde_json::to_vec(
                &serde_json::json!({ "items": self.items, "folders": self.folders }),
            )
            .map_err(err)?,
        )
    }
    pub fn folders(&self) -> Vec<Folder> {
        self.folders
            .iter()
            .map(|folder| Folder {
                count: self
                    .items
                    .iter()
                    .filter(|i| {
                        i.folder_id.as_deref() == Some(&folder.id) && self.path(i, false).is_file()
                    })
                    .count(),
                ..folder.clone()
            })
            .collect()
    }
    pub fn create_folder(&mut self, name: &str) -> Result<Folder> {
        self.add_folder(name, None)
    }
    /// A folder backed by a real directory; named after it, with a suffix if that name is taken.
    pub fn create_permanent_folder(&mut self, directory: &Path) -> Result<Folder> {
        let directory = directory.canonicalize().map_err(err)?;
        if !directory.is_dir() || directory.starts_with(self.root.canonicalize().map_err(err)?) {
            return Err("Choose an existing folder outside Captrain's private library.".into());
        }
        if self
            .folders
            .iter()
            .any(|f| f.path.as_deref() == Some(&directory))
        {
            return Err("That PC folder is already added.".into());
        }
        let base = directory
            .file_name()
            .map_or("Folder".into(), |n| n.to_string_lossy().into_owned());
        let mut name = base.clone();
        let mut n = 1;
        while self
            .folders
            .iter()
            .any(|f| f.name.to_lowercase() == name.to_lowercase())
        {
            n += 1;
            name = format!("{base} ({n})");
        }
        self.add_folder(&name, Some(directory))
    }
    /// Where a capture filed in `folder` is saved on disk: that folder's own directory if permanent, else `fallback`.
    pub fn export_dir(&self, folder: Option<&str>, fallback: Option<&Path>) -> Option<PathBuf> {
        folder
            .and_then(|id| self.folders.iter().find(|f| f.id == id))
            .and_then(|f| f.path.clone())
            .or_else(|| fallback.map(Path::to_path_buf))
    }
    fn add_folder(&mut self, name: &str, path: Option<PathBuf>) -> Result<Folder> {
        let name = valid_folder_name(name)?;
        if self
            .folders
            .iter()
            .any(|f| f.name.to_lowercase() == name.to_lowercase())
        {
            return Err("A folder with that name already exists.".into());
        }
        let folder = Folder {
            id: Uuid::new_v4().to_string(),
            name,
            path,
            count: 0,
        };
        self.folders.push(folder.clone());
        if let Err(error) = self.persist() {
            self.folders.pop();
            return Err(error);
        }
        Ok(folder)
    }
    pub fn rename_folder(&mut self, id: &str, name: &str) -> Result<()> {
        let name = valid_folder_name(name)?;
        if self
            .folders
            .iter()
            .any(|f| f.id != id && f.name.to_lowercase() == name.to_lowercase())
        {
            return Err("A folder with that name already exists.".into());
        }
        let index = self
            .folders
            .iter()
            .position(|f| f.id == id)
            .ok_or("Folder not found.")?;
        let previous = std::mem::replace(&mut self.folders[index].name, name);
        if let Err(error) = self.persist() {
            self.folders[index].name = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn delete_folder(&mut self, id: &str) -> Result<()> {
        let index = self
            .folders
            .iter()
            .position(|f| f.id == id)
            .ok_or("Folder not found.")?;
        let folder = self.folders.remove(index);
        let assigned: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.folder_id.as_deref() == Some(id))
            .map(|(n, _)| n)
            .collect();
        for &n in &assigned {
            self.items[n].folder_id = None;
        }
        if let Err(error) = self.persist() {
            for n in assigned {
                self.items[n].folder_id = Some(id.into());
            }
            self.folders.insert(index, folder);
            return Err(error);
        }
        Ok(())
    }
    pub fn move_capture(&mut self, id: &str, folder: Option<&str>) -> Result<()> {
        self.validate_folder(folder)?;
        let index = self
            .items
            .iter()
            .position(|i| i.id == id)
            .ok_or("Capture not found.")?;
        let previous =
            std::mem::replace(&mut self.items[index].folder_id, folder.map(String::from));
        if let Err(error) = self.persist() {
            self.items[index].folder_id = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn validate_folder(&self, folder: Option<&str>) -> Result<()> {
        if folder.is_some_and(|id| !self.folders.iter().any(|f| f.id == id)) {
            return Err("Folder no longer exists. Choose another folder.".into());
        }
        Ok(())
    }
    pub fn mark_exported(&mut self, id: &str, path: PathBuf) -> Result<Item> {
        let index = self
            .items
            .iter()
            .position(|i| i.id == id)
            .ok_or("Capture not found.")?;
        let previous = self.items[index].exported_path.replace(path);
        if let Err(error) = self.persist() {
            self.items[index].exported_path = previous;
            return Err(error);
        }
        Ok(self.items[index].clone())
    }
    pub fn auto_export(&mut self, id: &str, directory: &Path) -> Result<Item> {
        let (source, filename) = self.export_source(id)?;
        let directory = directory.canonicalize().map_err(err)?;
        if directory.starts_with(self.root.canonicalize().map_err(err)?) {
            return Err("Choose a save folder outside Captrain's private library.".into());
        }
        let destination = directory.join(format!(
            "{}-{}.{}",
            Path::new(&filename)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            id,
            source
                .extension()
                .and_then(|e| e.to_str())
                .ok_or("Capture has no extension.")?
        ));
        let named_link = self.find(id)?.kind == Kind::Url;
        let mut destination = if named_link {
            directory.join(&filename)
        } else {
            destination
        };
        let mut attempt = 1u64;
        let mut file = loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
            {
                Ok(file) => break file,
                Err(e) if named_link && e.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempt += 1;
                    let stem = Path::new(&filename)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy();
                    destination = directory.join(format!("{stem} ({attempt}).url"));
                }
                Err(e) => return Err(err(e)),
            }
        };
        let write = (|| {
            std::io::copy(&mut File::open(source).map_err(err)?, &mut file).map_err(err)?;
            file.sync_all().map_err(err)
        })();
        drop(file);
        if let Err(error) = write {
            let _ = fs::remove_file(&destination);
            return Err(error);
        }
        // A metadata failure leaves the completed export intact and reports its location.
        self.mark_exported(id, destination.clone()).map_err(|e| {
            format!(
                "Saved at {}, but export history could not update: {e}",
                destination.display()
            )
        })
    }
    pub fn save_unexported(&mut self, ids: &[String], directory: &Path) -> Result<BatchResult> {
        for id in ids {
            self.find(id)?;
        }
        let mut result = BatchResult::default();
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            if !seen.insert(id) {
                continue;
            }
            let item = self.find(id)?;
            let name = item.title.as_deref().unwrap_or(&item.filename).to_owned();
            if item.exported_path.is_some() {
                result.completed.push(id.clone());
                continue;
            }
            match self.auto_export(id, directory) {
                Ok(_) => result.completed.push(id.clone()),
                Err(e) => result.errors.push(format!("{name}: {e}")),
            }
        }
        Ok(result)
    }
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("captures")).map_err(err)?;
        fs::create_dir_all(root.join("thumbnails")).map_err(err)?;
        fs::create_dir_all(root.join("recordings")).map_err(err)?;
        let mut warning = None;
        let mut writable = true;
        let index = match fs::read(root.join("index.json")) {
            Ok(bytes) => {
                match serde_json::from_slice::<StoredIndex>(&bytes).map(StoredIndex::current) {
                    Ok(index)
                        if index.items.iter().all(|i| {
                            Uuid::parse_str(&i.id).is_ok()
                                && i.title
                                    .as_ref()
                                    .is_none_or(|title| valid_link_title(title).is_ok())
                                && if i.kind == Kind::Url {
                                    i.url
                                        .as_deref()
                                        .is_some_and(|u| crate::links::validate_link(u).is_ok())
                                } else {
                                    i.width > 0 && i.height > 0 && i.url.is_none()
                                }
                                && chrono::DateTime::parse_from_rfc3339(&i.created_at).is_ok()
                        }) && {
                            let mut ids = std::collections::HashSet::new();
                            index.items.iter().all(|i| ids.insert(&i.id))
                        } && {
                            let mut ids = std::collections::HashSet::new();
                            index.folders.iter().all(|f| {
                                Uuid::parse_str(&f.id).is_ok()
                                    && valid_folder_name(&f.name).is_ok()
                                    && ids.insert(&f.id)
                            })
                        } && index.items.iter().all(|i| {
                            i.folder_id
                                .as_ref()
                                .is_none_or(|id| index.folders.iter().any(|f| &f.id == id))
                        }) =>
                    {
                        index
                    }
                    _ => {
                        writable = false;
                        warning = Some("Capture index is damaged. It has been preserved; restore index.json from a backup before capturing again.".into());
                        Index::default()
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Index::default(),
            Err(e) => return Err(err(e)),
        };
        if index.items.iter().any(|i| {
            !root
                .join(if i.kind == Kind::Video {
                    "recordings"
                } else {
                    "captures"
                })
                .join(format!("{}.{}", i.id, i.kind.extension()))
                .is_file()
        }) {
            warning = Some("Some capture files are missing. Their metadata is preserved; only available captures are shown.".into());
        }
        let pending = fs::read_dir(root.join("recordings"))
            .map_err(err)?
            .filter_map(|e| e.ok())
            .any(|e| {
                e.path().extension().is_some_and(|x| x == "mp4")
                    && !index.items.iter().any(|i| {
                        i.kind == Kind::Video
                            && e.path().file_stem().is_some_and(|s| s == i.id.as_str())
                    })
            });
        if pending {
            warning = Some(format!("{}Interrupted or unindexed recordings were preserved in {}. Copy them elsewhere before attempting recovery.",
                warning.map(|w| format!("{w} ")).unwrap_or_default(), root.join("recordings").display()));
        }
        Ok(Self {
            root,
            items: index.items,
            folders: index.folders,
            warning,
            writable,
        })
    }
    pub fn list(&self, query: &str, offset: usize, limit: usize) -> Page {
        self.list_filtered(query, offset, limit, None, false)
    }
    pub fn list_filtered(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        folder: Option<&str>,
        exported_only: bool,
    ) -> Page {
        self.list_content(query, offset, limit, folder, exported_only, Content::All)
    }
    pub fn list_content(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        folder: Option<&str>,
        exported_only: bool,
        content: Content,
    ) -> Page {
        let q = query.to_lowercase();
        let terms: Vec<_> = q
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();
        let filtered: Vec<_> = self
            .items
            .iter()
            .rev()
            .filter(|i| {
                if (matches!(content, Content::Images) && !i.kind.is_image())
                    || (matches!(content, Content::Captures) && i.kind == Kind::Url)
                    || folder.is_some_and(|id| i.folder_id.as_deref() != Some(id))
                    || (exported_only && i.exported_path.is_none())
                {
                    return false;
                }
                if !self.path(i, false).is_file() {
                    return false;
                }
                let date = chrono::DateTime::parse_from_rfc3339(&i.created_at)
                    .map(|d| d.with_timezone(&Local).format("%x %B %e %Y").to_string())
                    .unwrap_or_default();
                let searchable = format!(
                    "{} {} {} {} {} {} {}",
                    i.kind.name(),
                    i.created_at,
                    date,
                    i.filename,
                    i.url.as_deref().unwrap_or(""),
                    i.title.as_deref().unwrap_or(""),
                    i.url
                        .as_deref()
                        .and_then(|url| crate::links::link_text(url).ok())
                        .unwrap_or_default()
                )
                .to_lowercase();
                terms.iter().all(|term| searchable.contains(term))
            })
            .collect();
        Page {
            total: filtered.len(),
            items: filtered
                .into_iter()
                .skip(offset)
                .take(limit.clamp(1, 100))
                .cloned()
                .collect(),
        }
    }
    fn path(&self, item: &Item, thumbnail: bool) -> PathBuf {
        self.root
            .join(if thumbnail {
                "thumbnails"
            } else if item.kind == Kind::Video {
                "recordings"
            } else {
                "captures"
            })
            .join(format!(
                "{}.{}",
                item.id,
                if thumbnail {
                    "png"
                } else {
                    item.kind.extension()
                }
            ))
    }
    fn find(&self, id: &str) -> Result<&Item> {
        self.items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| "Capture not found.".into())
    }
    pub fn image(&self, id: &str, thumbnail: bool) -> Result<String> {
        let item = self.find(id)?;
        if !item.kind.is_image() {
            return Err("This item does not have an image.".into());
        }
        let bytes = fs::read(self.path(item, thumbnail)).map_err(err)?;
        Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    }
    pub fn commit(
        &mut self,
        kind: Kind,
        image: DynamicImage,
        folder: Option<&str>,
    ) -> Result<Item> {
        if !kind.is_image() {
            return Err("Use the link or video transaction for this item.".into());
        }
        self.validate_folder(folder)?;
        if !self.writable {
            return Err("Capture index needs recovery; existing data was left untouched.".into());
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let (width, height) = image.dimensions();
        let item = Item {
            id,
            kind,
            created_at: now.to_rfc3339(),
            width,
            height,
            filename: format!(
                "Captrain-{}-{}.png",
                kind.name(),
                now.format("%Y%m%d-%H%M%S")
            ),
            folder_id: folder.map(String::from),
            exported_path: None,
            url: None,
            title: None,
        };
        let original = self.path(&item, false);
        let thumb = self.path(&item, true);
        let result = (|| {
            image
                .save_with_format(&original, image::ImageFormat::Png)
                .map_err(err)?;
            fs::OpenOptions::new()
                .write(true)
                .open(&original)
                .and_then(|f| f.sync_all())
                .map_err(err)?;
            image
                .thumbnail(160, 100)
                .save_with_format(&thumb, image::ImageFormat::Png)
                .map_err(err)?;
            fs::OpenOptions::new()
                .write(true)
                .open(&thumb)
                .and_then(|f| f.sync_all())
                .map_err(err)?;
            self.items.push(item.clone());
            let commit = self.persist();
            if commit.is_err() {
                self.items.pop();
            }
            commit
        })();
        if result.is_err() {
            let _ = fs::remove_file(original);
            let _ = fs::remove_file(thumb);
        }
        result.map(|_| item)
    }
    pub fn validate_recording(&self, folder: Option<&str>) -> Result<()> {
        if !self.writable {
            return Err("Capture index needs recovery; existing data was left untouched.".into());
        }
        self.validate_folder(folder)
    }
    pub fn is_video(&self, id: &str) -> Result<bool> {
        Ok(self.find(id)?.kind == Kind::Video)
    }
    pub fn commit_video(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        folder: Option<&str>,
    ) -> Result<Item> {
        self.validate_recording(folder)?;
        Uuid::parse_str(id).map_err(err)?;
        if width < 2 || height < 2 || self.items.iter().any(|i| i.id == id) {
            return Err("Invalid recording metadata.".into());
        }
        let now = Utc::now();
        let item = Item {
            id: id.into(),
            kind: Kind::Video,
            created_at: now.to_rfc3339(),
            width,
            height,
            filename: format!("Captrain-video-{}.mp4", now.format("%Y%m%d-%H%M%S")),
            folder_id: folder.map(String::from),
            exported_path: None,
            url: None,
            title: None,
        };
        let path = self.path(&item, false);
        if fs::metadata(&path).map_err(err)?.len() == 0 {
            return Err("The recorder produced an empty file.".into());
        }
        File::options()
            .write(true)
            .open(path)
            .and_then(|f| f.sync_all())
            .map_err(err)?;
        self.items.push(item.clone());
        if let Err(e) = self.persist() {
            self.items.pop();
            return Err(e);
        }
        Ok(item)
    }
    pub fn commit_url(
        &mut self,
        url: &str,
        title: Option<&str>,
        folder: Option<&str>,
    ) -> Result<Item> {
        let url = crate::links::validate_link(url)?;
        let parsed = tauri::Url::parse(&url).map_err(err)?;
        let fallback = if parsed.scheme() == "file" {
            let path = parsed
                .to_file_path()
                .map_err(|_| "This Windows path is invalid.")?;
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        } else {
            parsed.host_str().unwrap_or("Link").to_string()
        };
        let title = title
            .unwrap_or(&fallback)
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>();
        let title: String = title
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(120)
            .collect();
        let title = if title.trim().is_empty() {
            fallback.chars().take(120).collect()
        } else {
            title.trim().to_string()
        };
        if !self.writable {
            return Err("Capture index needs recovery; existing data was left untouched.".into());
        }
        self.validate_folder(folder)?;
        let now = Utc::now();
        let item = Item {
            id: Uuid::new_v4().to_string(),
            kind: Kind::Url,
            created_at: now.to_rfc3339(),
            width: 0,
            height: 0,
            filename: link_filename(&title),
            folder_id: folder.map(String::from),
            exported_path: None,
            url: Some(url.clone()),
            title: Some(title),
        };
        let path = self.path(&item, false);
        atomic_write(
            &path,
            format!("[InternetShortcut]\r\nURL={url}\r\n").as_bytes(),
        )?;
        self.items.push(item.clone());
        if let Err(e) = self.persist() {
            self.items.pop();
            let _ = fs::remove_file(path);
            return Err(e);
        }
        Ok(item)
    }
    pub fn rename_link(&mut self, id: &str, title: &str) -> Result<()> {
        if !self.writable {
            return Err("Capture index needs recovery.".into());
        }
        self.url(id)?;
        let title = valid_link_title(title)?;
        let index = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or("Link not found.")?;
        let previous = self.items[index].title.replace(title);
        if let Err(error) = self.persist() {
            self.items[index].title = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn url(&self, id: &str) -> Result<String> {
        crate::links::validate_link(
            self.find(id)?
                .url
                .as_deref()
                .ok_or("This item is not a link.")?,
        )
    }
    pub fn copy(&self, id: &str) -> Result<()> {
        if self.is_video(id)? {
            return Err("Use Save as to copy a recording to a file.".into());
        }
        if let Some(url) = &self.find(id)?.url {
            return arboard::Clipboard::new()
                .map_err(err)?
                .set_text(crate::links::link_text(url)?)
                .map_err(err);
        }
        let image = image::open(self.path(self.find(id)?, false))
            .map_err(err)?
            .into_rgba8();
        arboard::Clipboard::new()
            .map_err(err)?
            .set_image(arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: std::borrow::Cow::Owned(image.into_raw()),
            })
            .map_err(err)
    }
    pub fn get(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }
    pub fn export_source(&self, id: &str) -> Result<(PathBuf, String)> {
        let i = self.find(id)?;
        let filename = if i.kind == Kind::Url {
            let fallback = Path::new(&i.filename)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            link_filename(i.title.as_deref().unwrap_or(&fallback))
        } else {
            i.filename.clone()
        };
        Ok((self.path(i, false), filename))
    }
    pub fn delete(&mut self, id: &str) -> Result<()> {
        if !self.writable {
            return Err("Capture index needs recovery.".into());
        }
        let index = self
            .items
            .iter()
            .position(|i| i.id == id)
            .ok_or("Capture not found.")?;
        let item = self.items.remove(index);
        if let Err(e) = self.persist() {
            self.items.insert(index, item);
            return Err(e);
        }
        // The committed index is authoritative; a failed cleanup leaves an orphan, never loses another item.
        for path in [self.path(&item, false), self.path(&item, true)] {
            if let Err(e) = fs::remove_file(path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!(
                        "Removed from history, but file cleanup failed: {e}"
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_backup_excludes_pc_paths_and_restore_is_additive_and_transactional() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let other_root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut original = Library::open(root.clone()).unwrap();
        let folder = original.create_folder("Research").unwrap();
        let item = original
            .commit_url("https://example.com/", Some("Example"), Some(&folder.id))
            .unwrap();
        original.commit_url("file:///C:/Work/", None, None).unwrap();
        original.items[0].exported_path = Some(PathBuf::from("C:/private/export.url"));
        let backups = original.backup_items().unwrap();
        assert_eq!(backups.len(), 1);
        let backup = &backups[0];
        assert!(backup.item.exported_path.is_none());
        assert!(backup.folder.as_ref().unwrap().path.is_none());
        let source = root.join("staged.url");
        fs::copy(original.path(&item, false), &source).unwrap();
        let mut restored = Library::open(other_root.clone()).unwrap();
        assert!(restored.restore_backup(backup, &source, None).unwrap());
        restored.rename_link(&item.id, "Local edit").unwrap();
        assert!(!restored.restore_backup(backup, &source, None).unwrap());
        assert_eq!(
            restored.get(&item.id).unwrap().title.as_deref(),
            Some("Local edit")
        );
        assert_eq!(
            Library::open(other_root.clone()).unwrap().folders()[0].name,
            "Research"
        );
        let mut malformed = backup.clone();
        malformed.item.id = "../../outside".into();
        assert!(restored.restore_backup(&malformed, &source, None).is_err());
        malformed = backup.clone();
        malformed.item.url = Some("file:///C:/private/".into());
        assert!(malformed.validate().is_err());
        malformed = backup.clone();
        malformed.folder.as_mut().unwrap().path = Some(PathBuf::from("C:/private"));
        assert!(malformed.validate().is_err());
        let mut next = backup.clone();
        next.item.id = Uuid::new_v4().to_string();
        let orphan = restored.path(&next.item, false);
        fs::write(&orphan, b"keep orphan").unwrap();
        assert!(restored.restore_backup(&next, &source, None).is_err());
        assert_eq!(fs::read(&orphan).unwrap(), b"keep orphan");
        fs::remove_file(&orphan).unwrap();
        fs::copy(original.path(&item, false), &source).unwrap();
        // Force index replacement to fail after originals have been installed.
        fs::remove_file(other_root.join("index.json")).unwrap();
        fs::create_dir(other_root.join("index.json")).unwrap();
        assert!(restored.restore_backup(&next, &source, None).is_err());
        assert!(restored.get(&next.item.id).is_none());
        assert!(!restored.path(&next.item, false).exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other_root).unwrap();
    }

    #[test]
    fn cloud_backup_removes_permanent_folder_membership_and_rejects_damaged_index() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut library = Library::open(root.clone()).unwrap();
        let folder = library.create_folder("PC folder").unwrap();
        library.folders[0].path = Some(PathBuf::from("C:/private"));
        library
            .commit_url("https://example.com/", None, Some(&folder.id))
            .unwrap();
        let backups = library.backup_items().unwrap();
        assert!(backups[0].folder.is_none());
        assert!(backups[0].item.folder_id.is_none());
        fs::write(root.join("index.json"), b"broken").unwrap();
        assert!(Library::open(root.clone()).unwrap().backup_items().is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn videos_keep_owned_files_and_stream_exports_without_entering_image_navigation() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let exports = std::env::temp_dir().join(Uuid::new_v4().to_string());
        fs::create_dir(&exports).unwrap();
        let mut l = Library::open(root.clone()).unwrap();
        let id = Uuid::new_v4().to_string();
        let path = root.join("recordings").join(format!("{id}.mp4"));
        assert!(l.commit_video(&id, 1920, 1080, None).is_err());
        // The media engine validates/finalizes MP4; this test exercises storage transactions.
        fs::write(&path, b"recording fixture").unwrap();
        assert!(Library::open(root.clone()).unwrap().warning.is_some());
        assert!(l.commit_video("../foreign", 1920, 1080, None).is_err());
        let folder = l.create_folder("Videos").unwrap();
        let item = l.commit_video(&id, 1920, 1080, Some(&folder.id)).unwrap();
        assert!(l.commit_video(&id, 1920, 1080, None).is_err());
        assert!(l.is_video(&item.id).unwrap());
        assert!(l.image(&item.id, false).is_err());
        assert_eq!(
            l.list_content("", 0, 20, None, false, Content::Images)
                .total,
            0
        );
        assert_eq!(
            l.list_content("", 0, 20, None, false, Content::Captures)
                .total,
            1
        );
        let exported = l
            .auto_export(&item.id, &exports)
            .unwrap()
            .exported_path
            .unwrap();
        assert_eq!(exported.extension().unwrap(), "mp4");
        assert_eq!(fs::read(&exported).unwrap(), b"recording fixture");
        let mut l = Library::open(root.clone()).unwrap();
        assert!(l.warning.is_none());
        assert_eq!(l.folders()[0].count, 1);
        l.move_capture(&item.id, None).unwrap();
        l.delete(&item.id).unwrap();
        assert!(!path.exists());
        assert!(exported.exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(exports).unwrap();
    }
    #[test]
    fn failed_stream_copy_preserves_existing_destination() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        let destination = root.join("saved.mp4");
        fs::write(&destination, b"existing export").unwrap();
        assert!(atomic_copy(&root.join("missing.mp4"), &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"existing export");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn links_persist_search_move_export_and_delete() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut l = Library::open(root.clone()).unwrap();
        let folder = l.create_folder("Research").unwrap();
        let item = l
            .commit_url("https://example.com/path?q=one#two", None, Some(&folder.id))
            .unwrap();
        assert!(l.commit_url("javascript:alert(1)", None, None).is_err());
        assert_eq!(l.list("example", 0, 20).total, 1);
        assert_eq!(
            l.list_content("", 0, 20, None, false, Content::Images)
                .total,
            0
        );
        assert_eq!(l.folders()[0].count, 1);
        assert!(l.image(&item.id, false).is_err());
        assert!(fs::read_to_string(l.export_source(&item.id).unwrap().0)
            .unwrap()
            .contains("URL=https://example.com/"));
        let mut l = Library::open(root.clone()).unwrap();
        assert_eq!(
            l.url(&item.id).unwrap(),
            "https://example.com/path?q=one#two"
        );
        l.move_capture(&item.id, None).unwrap();
        // Keep export outside the private library, as required by the production path.
        let exports = root.with_extension("link-exports");
        fs::create_dir(&exports).unwrap();
        let exported = l
            .auto_export(&item.id, &exports)
            .unwrap()
            .exported_path
            .unwrap();
        assert_eq!(exported.extension().unwrap(), "url");
        l.delete(&item.id).unwrap();
        assert!(exported.is_file());
        fs::remove_dir_all(exports).unwrap();
        assert_eq!(l.list("", 0, 20).total, 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn folder_links_and_renamed_titles_preserve_the_target() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let target = root.join("My Projects");
        fs::create_dir_all(&target).unwrap();
        let url = tauri::Url::from_directory_path(&target)
            .unwrap()
            .to_string();
        let mut library = Library::open(root.clone()).unwrap();
        let item = library.commit_url(&url, None, None).unwrap();
        assert_eq!(item.title.as_deref(), Some("My Projects"));
        assert_eq!(item.filename, "My Projects.url");
        library.rename_link(&item.id, "Game source").unwrap();
        let mut library = Library::open(root.clone()).unwrap();
        assert_eq!(
            library.get(&item.id).unwrap().title.as_deref(),
            Some("Game source")
        );
        assert_eq!(library.list("game source", 0, 20).total, 1);
        assert_eq!(library.list(target.to_str().unwrap(), 0, 20).total, 1);
        assert_eq!(library.url(&item.id).unwrap(), url);
        for invalid in ["", "  ", "bad\nname"] {
            assert!(library.rename_link(&item.id, invalid).is_err());
        }
        assert!(library.rename_link(&item.id, &"a".repeat(121)).is_err());
        fs::rename(root.join("index.json"), root.join("held-index")).unwrap();
        fs::create_dir(root.join("index.json")).unwrap();
        assert!(library.rename_link(&item.id, "Cannot persist").is_err());
        assert_eq!(
            library.get(&item.id).unwrap().title.as_deref(),
            Some("Game source")
        );
        fs::remove_dir(root.join("index.json")).unwrap();
        fs::rename(root.join("held-index"), root.join("index.json")).unwrap();
        library.delete(&item.id).unwrap();
        assert!(target.is_dir());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn captured_names_persist_and_exports_do_not_overwrite() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let exports = root.with_extension("exports");
        fs::create_dir(&exports).unwrap();
        let mut library = Library::open(root.clone()).unwrap();
        let item = library
            .commit_url(
                "https://example.com/page",
                Some("  Actual page: title?  "),
                None,
            )
            .unwrap();
        assert_eq!(item.title.as_deref(), Some("Actual page: title?"));
        assert_eq!(item.filename, "Actual page_ title_.url");
        let mut library = Library::open(root.clone()).unwrap();
        assert_eq!(library.get(&item.id).unwrap().title, item.title);
        assert_eq!(library.list("Actual page", 0, 20).total, 1);
        let first = library
            .auto_export(&item.id, &exports)
            .unwrap()
            .exported_path
            .unwrap();
        let second = library
            .auto_export(&item.id, &exports)
            .unwrap()
            .exported_path
            .unwrap();
        assert_eq!(first.file_name().unwrap(), "Actual page_ title_.url");
        assert_eq!(second.file_name().unwrap(), "Actual page_ title_ (2).url");
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        library.rename_link(&item.id, "New name").unwrap();
        assert_eq!(library.export_source(&item.id).unwrap().1, "New name.url");
        assert!(first.is_file());
        let fallback = library
            .commit_url("https://example.com/", Some("\n\t"), None)
            .unwrap();
        assert_eq!(fallback.title.as_deref(), Some("example.com"));
        let long = library
            .commit_url("https://example.com/", Some(&"🦀".repeat(200)), None)
            .unwrap();
        assert_eq!(long.title.unwrap().chars().count(), 120);
        for (title, filename) in [
            ("CON", "_CON.url"),
            ("CON .txt", "_CON .txt.url"),
            ("aux.txt", "_aux.txt.url"),
            ("LPT¹", "_LPT¹.url"),
            ("../", "_.url"),
            ("...", "Link.url"),
        ] {
            assert_eq!(link_filename(title), filename);
        }
        fs::remove_dir_all(exports).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn batch_save_skips_exports_validates_ids_and_reports_failures() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let exports = root.with_extension("exports");
        fs::create_dir(&exports).unwrap();
        let mut library = Library::open(root.clone()).unwrap();
        let first = library
            .commit_url("https://example.com/one", Some("Same name"), None)
            .unwrap();
        let second = library
            .commit_url("https://example.com/two", Some("Same name"), None)
            .unwrap();
        assert!(library
            .save_unexported(&[first.id.clone(), "foreign".into()], &exports)
            .is_err());
        assert_eq!(fs::read_dir(&exports).unwrap().count(), 0);
        let failure = library
            .save_unexported(
                &[first.id.clone(), second.id.clone()],
                &exports.join("missing"),
            )
            .unwrap();
        assert_eq!(failure.errors.len(), 2);
        assert!(failure.completed.is_empty());
        assert!(library.get(&first.id).unwrap().exported_path.is_none());
        library.auto_export(&first.id, &exports).unwrap();
        let result = library
            .save_unexported(
                &[first.id.clone(), second.id.clone(), second.id.clone()],
                &exports,
            )
            .unwrap();
        assert_eq!(result.completed, vec![first.id, second.id]);
        assert!(result.errors.is_empty());
        assert_eq!(fs::read_dir(&exports).unwrap().count(), 2);
        assert!(exports.join("Same name.url").is_file());
        assert!(exports.join("Same name (2).url").is_file());
        fs::remove_dir_all(exports).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn web_link_names_survive_reload_and_legacy_items_need_no_migration() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut library = Library::open(root.clone()).unwrap();
        let item = library
            .commit_url("https://example.com/", None, None)
            .unwrap();
        let mut index: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("index.json")).unwrap()).unwrap();
        index["items"][0].as_object_mut().unwrap().remove("title");
        index["items"][0]["filename"] = "../../outside.url".into();
        fs::write(root.join("index.json"), serde_json::to_vec(&index).unwrap()).unwrap();
        let mut library = Library::open(root.clone()).unwrap();
        assert!(library.get(&item.id).unwrap().title.is_none());
        assert_eq!(library.export_source(&item.id).unwrap().1, "outside.url");
        library.rename_link(&item.id, "Reference").unwrap();
        let library = Library::open(root.clone()).unwrap();
        assert_eq!(
            library.get(&item.id).unwrap().title.as_deref(),
            Some("Reference")
        );
        assert_eq!(library.url(&item.id).unwrap(), "https://example.com/");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn permanent_folder_saves_to_its_own_directory() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let pc = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let fallback = std::env::temp_dir().join(Uuid::new_v4().to_string());
        fs::create_dir_all(&pc).unwrap();
        let mut library = Library::open(root.clone()).unwrap();
        let temp = library.create_folder("Temp").unwrap();
        let perm = library.create_permanent_folder(&pc).unwrap();
        assert!(library.create_permanent_folder(&pc).is_err());
        assert!(library
            .create_permanent_folder(&root.join("captures"))
            .is_err());
        assert_eq!(
            library.export_dir(Some(&perm.id), Some(&fallback)),
            Some(pc.canonicalize().unwrap())
        );
        assert_eq!(
            library.export_dir(Some(&temp.id), Some(&fallback)),
            Some(fallback.clone())
        );
        assert_eq!(library.export_dir(None, None), None);
        let reopened = Library::open(root.clone()).unwrap();
        assert!(reopened.folders().iter().any(|f| f.path.is_some()));
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(pc).unwrap();
    }
    #[test]
    fn folders_migrate_legacy_and_ungroup_without_deleting_images() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut library = Library::open(root.clone()).unwrap();
        let item = library
            .commit(Kind::Screen, DynamicImage::new_rgba8(4, 3), None)
            .unwrap();
        let mut legacy = serde_json::to_value(&library.items).unwrap();
        legacy[0].as_object_mut().unwrap().remove("folderId");
        legacy[0].as_object_mut().unwrap().remove("exportedPath");
        fs::write(
            root.join("index.json"),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();
        let mut library = Library::open(root.clone()).unwrap();
        assert_eq!(library.list("", 0, 20).total, 1);
        let folder = library.create_folder(" Ideas ").unwrap();
        assert_eq!(folder.name, "Ideas");
        assert!(library.create_folder("ideas").is_err());
        assert!(library.create_folder("  ").is_err());
        assert!(library.move_capture(&item.id, Some("../foreign")).is_err());
        library.move_capture(&item.id, Some(&folder.id)).unwrap();
        library.rename_folder(&folder.id, "Inspiration").unwrap();
        let mut reopened = Library::open(root.clone()).unwrap();
        assert_eq!(reopened.folders()[0].name, "Inspiration");
        assert_eq!(reopened.folders()[0].count, 1);
        assert_eq!(
            reopened
                .list_filtered("", 0, 20, Some(&folder.id), false)
                .total,
            1
        );
        reopened.delete_folder(&folder.id).unwrap();
        assert_eq!(reopened.list("", 0, 20).items[0].folder_id, None);
        assert!(reopened.image(&item.id, false).is_ok());
        assert!(Library::open(root.clone()).unwrap().folders().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn auto_save_is_unique_and_library_delete_preserves_export() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let output = std::env::temp_dir().join(Uuid::new_v4().to_string());
        fs::create_dir_all(&output).unwrap();
        let mut library = Library::open(root.clone()).unwrap();
        let folder = library.create_folder("Work").unwrap();
        let item = library
            .commit(
                Kind::Region,
                DynamicImage::new_rgba8(4, 3),
                Some(&folder.id),
            )
            .unwrap();
        assert!(library
            .auto_export(&item.id, &root.join("captures"))
            .is_err());
        assert!(library
            .auto_export(&item.id, &output.join("missing"))
            .is_err());
        assert_eq!(library.list_filtered("", 0, 20, None, true).total, 0);
        let exported = library.auto_export(&item.id, &output).unwrap();
        let path = exported.exported_path.unwrap();
        assert_eq!(image::open(&path).unwrap().dimensions(), (4, 3));
        assert!(library.auto_export(&item.id, &output).is_err());
        assert_eq!(
            Library::open(root.clone())
                .unwrap()
                .list_filtered("", 0, 20, None, true)
                .total,
            1
        );
        library.delete(&item.id).unwrap();
        assert!(path.is_file());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(output).unwrap();
    }
    #[test]
    fn commit_reopen_delete_and_reject_foreign_ids() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut l = Library::open(root.clone()).unwrap();
        let item = l
            .commit(Kind::Region, DynamicImage::new_rgba8(12, 8), None)
            .unwrap();
        let displayed_date = chrono::DateTime::parse_from_rfc3339(&item.created_at)
            .unwrap()
            .with_timezone(&Local)
            .format("%b %-d, %Y")
            .to_string();
        assert_eq!(l.list(&displayed_date, 0, 20).total, 1);
        assert!(l.image("../../elsewhere", false).is_err());
        assert_eq!(
            Library::open(root.clone())
                .unwrap()
                .list("region", 0, 20)
                .total,
            1
        );
        l.delete(&item.id).unwrap();
        assert_eq!(
            Library::open(root.clone()).unwrap().list("", 0, 20).total,
            0
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_index_is_preserved() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.json"), b"broken").unwrap();
        let mut l = Library::open(root.clone()).unwrap();
        assert!(l
            .commit(Kind::Screen, DynamicImage::new_rgba8(1, 1), None)
            .is_err());
        assert_eq!(fs::read(root.join("index.json")).unwrap(), b"broken");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn paginates_a_thousand_without_decoding_originals() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut l = Library::open(root.clone()).unwrap();
        for n in 0..1000 {
            let id = Uuid::new_v4().to_string();
            fs::write(root.join("captures").join(format!("{id}.png")), []).unwrap();
            l.items.push(Item {
                id,
                kind: Kind::Screen,
                created_at: Utc::now().to_rfc3339(),
                width: 1,
                height: 1,
                filename: format!("capture-{n}.png"),
                folder_id: None,
                exported_path: None,
                url: None,
                title: None,
            });
        }
        let page = l.list("screen", 20, 20);
        assert_eq!(page.total, 1000);
        assert_eq!(page.items.len(), 20);
        fs::remove_dir_all(root).unwrap();
    }
}
