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
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Region,
    Window,
    Screen,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Window => "window",
            Self::Screen => "screen",
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
}
#[derive(Serialize)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: usize,
}

pub struct Library {
    pub root: PathBuf,
    items: Vec<Item>,
    pub warning: Option<String>,
    writable: bool,
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = File::create(&temp).map_err(err)?;
        file.write_all(bytes).map_err(err)?;
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

impl Library {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("captures")).map_err(err)?;
        fs::create_dir_all(root.join("thumbnails")).map_err(err)?;
        let mut warning = None;
        let mut writable = true;
        let items = match fs::read(root.join("index.json")) {
            Ok(bytes) => match serde_json::from_slice::<Vec<Item>>(&bytes) {
                Ok(items)
                    if items.iter().all(|i| {
                        Uuid::parse_str(&i.id).is_ok()
                            && i.width > 0
                            && i.height > 0
                            && chrono::DateTime::parse_from_rfc3339(&i.created_at).is_ok()
                    }) && {
                        let mut ids = std::collections::HashSet::new();
                        items.iter().all(|i| ids.insert(&i.id))
                    } =>
                {
                    items
                }
                _ => {
                    writable = false;
                    warning = Some("Capture index is damaged. It has been preserved; restore index.json from a backup before capturing again.".into());
                    vec![]
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(err(e)),
        };
        if items.iter().any(|i| {
            !root
                .join("captures")
                .join(format!("{}.png", i.id))
                .is_file()
        }) {
            warning = Some("Some capture files are missing. Their metadata is preserved; only available captures are shown.".into());
        }
        Ok(Self {
            root,
            items,
            warning,
            writable,
        })
    }
    pub fn list(&self, query: &str, offset: usize, limit: usize) -> Page {
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
                if !self.path(i, false).is_file() {
                    return false;
                }
                let date = chrono::DateTime::parse_from_rfc3339(&i.created_at)
                    .map(|d| d.with_timezone(&Local).format("%x %B %e %Y").to_string())
                    .unwrap_or_default();
                let searchable =
                    format!("{} {} {} {}", i.kind.name(), i.created_at, date, i.filename)
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
            .join(if thumbnail { "thumbnails" } else { "captures" })
            .join(format!("{}.png", item.id))
    }
    fn find(&self, id: &str) -> Result<&Item> {
        self.items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| "Capture not found.".into())
    }
    pub fn image(&self, id: &str, thumbnail: bool) -> Result<String> {
        let item = self.find(id)?;
        let bytes = fs::read(self.path(item, thumbnail)).map_err(err)?;
        Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    }
    pub fn commit(&mut self, kind: Kind, image: DynamicImage) -> Result<Item> {
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
            let commit = atomic_write(
                &self.root.join("index.json"),
                &serde_json::to_vec(&self.items).map_err(err)?,
            );
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
    pub fn copy(&self, id: &str) -> Result<()> {
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
    pub fn export_source(&self, id: &str) -> Result<(PathBuf, String)> {
        let i = self.find(id)?;
        Ok((self.path(i, false), i.filename.clone()))
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
        if let Err(e) = atomic_write(
            &self.root.join("index.json"),
            &serde_json::to_vec(&self.items).map_err(err)?,
        ) {
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
    fn commit_reopen_delete_and_reject_foreign_ids() {
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut l = Library::open(root.clone()).unwrap();
        let item = l
            .commit(Kind::Region, DynamicImage::new_rgba8(12, 8))
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
            .commit(Kind::Screen, DynamicImage::new_rgba8(1, 1))
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
            });
        }
        let page = l.list("screen", 20, 20);
        assert_eq!(page.total, 1000);
        assert_eq!(page.items.len(), 20);
        fs::remove_dir_all(root).unwrap();
    }
}
