mod auth;
mod drive;

use crate::{
    library::{atomic_write, err, BackupItem, Kind, Result},
    shelf_only, AppState,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    images: bool,
    links: bool,
    videos: bool,
    include_existing: bool,
    paused: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            images: true,
            links: true,
            videos: false,
            include_existing: false,
            paused: false,
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    options: Options,
    since: Option<String>,
    last_backup: Option<String>,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    busy: bool,
    phase: String,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    configured: bool,
    connected: bool,
    account: Option<String>,
    options: Options,
    last_backup: Option<String>,
    #[serde(flatten)]
    progress: Progress,
}

pub struct Cloud {
    root: PathBuf,
    progress: Mutex<Progress>,
    busy: AtomicBool,
    cancel: AtomicBool,
    last_snapshot: Mutex<Option<Vec<u8>>>,
}

impl Cloud {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            progress: Mutex::new(Progress::default()),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            last_snapshot: Mutex::new(None),
        }
    }
    fn config(&self) -> Result<Config> {
        match fs::read(self.root.join("cloud-settings.json")) {
            Ok(bytes) => {
                let config: Config = serde_json::from_slice(&bytes).map_err(|_| "Cloud settings are damaged. Preserve and remove cloud-settings.json before trying again.")?;
                if config
                    .since
                    .as_deref()
                    .is_some_and(|v| chrono::DateTime::parse_from_rfc3339(v).is_err())
                {
                    return Err("Cloud backup cutoff is damaged. Preserve and remove cloud-settings.json before trying again.".into());
                }
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(err(e)),
        }
    }
    fn save_config(&self, config: &Config) -> Result<()> {
        atomic_write(
            &self.root.join("cloud-settings.json"),
            &serde_json::to_vec(config).map_err(err)?,
        )
    }
    fn status(&self) -> Result<Status> {
        let config = self.config()?;
        let credentials = auth::load(&self.root)?;
        Ok(Status {
            configured: credentials.is_some(),
            connected: credentials
                .as_ref()
                .is_some_and(|c| c.refresh_token.is_some()),
            account: credentials.and_then(|c| c.email),
            options: config.options,
            last_backup: config.last_backup,
            progress: self.progress.lock().unwrap().clone(),
        })
    }
    fn reserve(&self) -> Result<OperationGuard<'_>> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "A cloud operation is already running.")?;
        self.cancel.store(false, Ordering::Release);
        Ok(OperationGuard(self))
    }
    fn progress(&self, phase: impl Into<String>) {
        *self.progress.lock().unwrap() = Progress {
            busy: true,
            phase: phase.into(),
            error: None,
        };
    }
    fn cancelled(&self) -> Result<()> {
        if self.cancel.load(Ordering::Acquire) {
            Err(
                "Cloud operation cancelled. Local files and completed cloud copies are safe."
                    .into(),
            )
        } else {
            Ok(())
        }
    }
}
struct OperationGuard<'a>(&'a Cloud);
impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        self.0.progress.lock().unwrap().busy = false;
        self.0.busy.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub fn cloud_status(window: WebviewWindow, cloud: tauri::State<Arc<Cloud>>) -> Result<Status> {
    shelf_only(&window)?;
    cloud.status()
}

#[tauri::command]
pub async fn cloud_configure(
    window: WebviewWindow,
    cloud: tauri::State<'_, Arc<Cloud>>,
) -> Result<Status> {
    shelf_only(&window)?;
    let cloud = cloud.inner().clone();
    let guard = cloud.reserve()?;
    if auth::load(&cloud.root)?.is_some_and(|c| c.refresh_token.is_some()) {
        return Err("Disconnect Google Drive before replacing its OAuth configuration.".into());
    }
    let picked = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Choose Google Desktop OAuth JSON")
        .add_filter("Google OAuth configuration", &["json"])
        .pick_file()
        .await;
    if let Some(file) = picked {
        let path = file.path().to_path_buf();
        let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>> {
            use std::io::Read;
            let mut bytes = Vec::new();
            File::open(path)
                .map_err(err)?
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(err)?;
            if bytes.len() > 65536 {
                return Err("OAuth configuration is too large.".into());
            }
            Ok(bytes)
        })
        .await
        .map_err(err)??;
        let credentials = auth::Credentials {
            client: auth::parse_client(&bytes)?,
            refresh_token: None,
            email: None,
        };
        auth::store(&cloud.root, &credentials)?;
        *cloud.progress.lock().unwrap() = Progress::default();
    }
    drop(guard);
    cloud.status()
}

#[tauri::command]
pub fn cloud_options(
    window: WebviewWindow,
    cloud: tauri::State<Arc<Cloud>>,
    options: Options,
) -> Result<Status> {
    shelf_only(&window)?;
    let guard = cloud.reserve()?;
    let mut config = cloud.config()?;
    if options.include_existing {
        config.since = None;
    } else if config.options.include_existing {
        config.since = Some(chrono::Utc::now().to_rfc3339());
    }
    config.options = options;
    cloud.save_config(&config)?;
    *cloud.progress.lock().unwrap() = Progress {
        busy: false,
        phase: if config.options.paused {
            "Backup paused".into()
        } else {
            "Backup preferences saved".into()
        },
        error: None,
    };
    *cloud.last_snapshot.lock().unwrap() = None;
    drop(guard);
    cloud.status()
}

#[tauri::command]
pub fn cloud_action(window: WebviewWindow, app: AppHandle, action: String) -> Result<Status> {
    shelf_only(&window)?;
    let cloud = app.state::<Arc<Cloud>>().inner().clone();
    if action == "cancel" {
        cloud.cancel.store(true, Ordering::Release);
    } else if action == "disconnect" {
        let guard = cloud.reserve()?;
        if let Some(mut credentials) = auth::load(&cloud.root)? {
            credentials.refresh_token = None;
            credentials.email = None;
            auth::store(&cloud.root, &credentials)?;
        }
        let mut config = cloud.config()?;
        config.last_backup = None;
        cloud.save_config(&config)?;
        *cloud.last_snapshot.lock().unwrap() = None;
        *cloud.progress.lock().unwrap() = Progress::default();
        drop(guard);
    } else {
        if !matches!(action.as_str(), "connect" | "backup" | "restore") {
            return Err("Unknown cloud action.".into());
        }
        start(&app, &action, false)?;
    }
    cloud.status()
}

fn selected(backup: &BackupItem, config: &Config, apply_cutoff: bool) -> bool {
    let kind = backup.item.kind;
    let selected = if kind.is_image() {
        config.options.images
    } else if kind == Kind::Url {
        config.options.links
    } else {
        config.options.videos
    };
    selected
        && (!apply_cutoff
            || config.since.as_ref().is_none_or(|cutoff| {
                chrono::DateTime::parse_from_rfc3339(&backup.item.created_at).ok()
                    >= chrono::DateTime::parse_from_rfc3339(cutoff).ok()
            }))
}

fn start(app: &AppHandle, action: &str, automatic: bool) -> Result<()> {
    let cloud = app.state::<Arc<Cloud>>().inner().clone();
    // Reserve before spawning so two UI invocations cannot queue competing account operations.
    let guard = cloud.reserve()?;
    let credentials =
        auth::load(&cloud.root)?.ok_or("Import your Google Desktop OAuth configuration first.")?;
    let mut config = cloud.config()?;
    if action != "connect" && credentials.refresh_token.is_none() {
        return Err("Connect Google Drive first.".into());
    }
    if action == "backup" && config.options.paused {
        return Err("Resume cloud backup first.".into());
    }
    if action == "connect"
        && !config.options.include_existing
        && credentials.refresh_token.is_none()
    {
        config.since = Some(chrono::Utc::now().to_rfc3339());
        cloud.save_config(&config)?;
    }
    // The worker owns this reservation until it finishes.
    std::mem::forget(guard);
    cloud.progress(match action {
        "connect" => "Finish sign-in in your browser".to_string(),
        "restore" => "Looking for cloud backups".into(),
        _ => "Checking backup".into(),
    });
    let action = action.to_string();
    let app = app.clone();
    std::thread::spawn(move || {
        let guard = OperationGuard(&cloud);
        let result = run(&app, &cloud, &action, automatic, credentials, config);
        let mut progress = cloud.progress.lock().unwrap();
        match result {
            Ok(message) => {
                progress.phase = message;
                progress.error = None;
            }
            Err(error) => {
                progress.phase = "Needs attention".into();
                progress.error = Some(error);
            }
        }
        progress.busy = false;
        drop(progress);
        drop(guard);
    });
    Ok(())
}

fn run(
    app: &AppHandle,
    cloud: &Cloud,
    action: &str,
    automatic: bool,
    mut credentials: auth::Credentials,
    mut config: Config,
) -> Result<String> {
    if action == "connect" {
        let previous_account = credentials.email.clone();
        auth::connect(&mut credentials, &cloud.cancel)?;
        cloud.cancelled()?;
        if previous_account.is_some() && previous_account != credentials.email {
            return Err("Disconnect the current Google account before connecting a different one. Its existing backups were left unchanged.".into());
        }
        auth::store(&cloud.root, &credentials)?;
        *cloud.last_snapshot.lock().unwrap() = None;
        return Ok("Connected. Selected items will back up automatically within a minute.".into());
    }
    let state = app.state::<Arc<AppState>>();
    let snapshot = if action == "backup" {
        let items: Vec<_> = state
            .library
            .lock()
            .unwrap()
            .backup_items()?
            .into_iter()
            .filter(|i| selected(i, &config, true))
            .collect();
        let signature = serde_json::to_vec(&items).map_err(err)?;
        if automatic && cloud.last_snapshot.lock().unwrap().as_ref() == Some(&signature) {
            return Ok("Up to date".into());
        }
        Some((items, signature))
    } else {
        None
    };
    let token = auth::access(&auth::http()?, &mut credentials, &cloud.root)?;
    let drive = drive::Drive::new(token)?;
    let files = drive.list()?;
    if let Some((items, signature)) = snapshot {
        let mut remote = std::collections::HashMap::new();
        for file in &files {
            let record = file.record()?;
            remote
                .entry(record.backup.item.id.clone())
                .or_insert((file, record));
        }
        let total = items.len();
        for (index, backup) in items.into_iter().enumerate() {
            cloud.cancelled()?;
            backup.validate()?;
            cloud.progress(format!("Backing up {} of {total}", index + 1));
            let source = state
                .library
                .lock()
                .unwrap()
                .export_source(&backup.item.id)
                .map(|v| v.0);
            let source = match source {
                Ok(source) => source,
                Err(_) => continue,
            }; // Deleted since this scan; cloud copies remain.
            let mut file = File::open(source).map_err(err)?;
            let hash = drive::digest(&mut file)?;
            let record = drive::Record {
                version: 1,
                backup,
                sha256: hash,
            };
            match remote.get(&record.backup.item.id) {
                Some((file, old)) if old.sha256 == record.sha256 => {
                    if serde_json::to_vec(&old.backup).map_err(err)? != serde_json::to_vec(&record.backup).map_err(err)? { drive.update_metadata(&file.id, &record)?; }
                }
                Some(_) => return Err("A cloud item has different content under the same ID. Neither copy was replaced.".into()),
                None => drive.upload(&record, &mut file, &cloud.cancel)?,
            }
        }
        cloud.cancelled()?;
        config.last_backup = Some(chrono::Utc::now().to_rfc3339());
        cloud.save_config(&config)?;
        *cloud.last_snapshot.lock().unwrap() = Some(signature);
        Ok("Up to date".into())
    } else {
        restore(app, cloud, &drive, files, &config)
    }
}

struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("original"));
        let _ = fs::remove_file(self.0.join("thumbnail.png"));
        let _ = fs::remove_dir(&self.0);
    }
}

fn restore(
    app: &AppHandle,
    cloud: &Cloud,
    drive: &drive::Drive,
    files: Vec<drive::RemoteFile>,
    config: &Config,
) -> Result<String> {
    let state = app.state::<Arc<AppState>>();
    let mut seen = HashSet::new();
    let mut restored = 0;
    let mut skipped = 0;
    for (index, remote) in files.iter().enumerate() {
        cloud.cancelled()?;
        let record = remote.record()?;
        let backup = record.backup;
        if !seen.insert(backup.item.id.clone()) || !selected(&backup, config, false) {
            continue;
        }
        if state.library.lock().unwrap().get(&backup.item.id).is_some() {
            skipped += 1;
            continue;
        }
        cloud.progress(format!(
            "Restoring cloud item {} of {} · {restored} added",
            index + 1,
            files.len()
        ));
        let stage = Staging(
            cloud
                .root
                .join(format!("cloud-restore-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir(&stage.0).map_err(err)?;
        let original = stage.0.join("original");
        drive.download(remote, &original)?;
        if drive::digest(&mut File::open(&original).map_err(err)?)? != record.sha256 {
            return Err(
                "Cloud file integrity check failed. Nothing from that file was imported.".into(),
            );
        }
        let thumb = prepare_restore(&backup, &original, &stage.0)?;
        cloud.cancelled()?;
        if state
            .library
            .lock()
            .unwrap()
            .restore_backup(&backup, &original, thumb.as_deref())?
        {
            restored += 1;
            crate::desktop::changed(app);
        } else {
            skipped += 1;
        }
    }
    Ok(format!(
        "Restored {restored} items; kept {skipped} existing items unchanged"
    ))
}

fn prepare_restore(
    backup: &BackupItem,
    original: &std::path::Path,
    staging: &std::path::Path,
) -> Result<Option<PathBuf>> {
    if backup.item.kind.is_image() {
        let mut reader = image::ImageReader::open(original)
            .map_err(err)?
            .with_guessed_format()
            .map_err(err)?;
        if reader.format() != Some(image::ImageFormat::Png) {
            return Err("Cloud screenshot is not a PNG.".into());
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(32768);
        limits.max_image_height = Some(32768);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(|_| {
            "Cloud screenshot is invalid or exceeds the 256 MiB restore decode limit."
        })?;
        if image.width() != backup.item.width || image.height() != backup.item.height {
            return Err("Cloud screenshot dimensions do not match its metadata.".into());
        }
        let thumb = staging.join("thumbnail.png");
        image
            .thumbnail(160, 100)
            .save_with_format(&thumb, image::ImageFormat::Png)
            .map_err(err)?;
        File::options()
            .write(true)
            .open(&thumb)
            .and_then(|file| file.sync_all())
            .map_err(err)?;
        Ok(Some(thumb))
    } else if backup.item.kind == Kind::Url {
        use std::io::Read;
        let expected = format!(
            "[InternetShortcut]\r\nURL={}\r\n",
            backup.item.url.as_deref().ok_or("Missing URL.")?
        );
        let mut bytes = Vec::new();
        File::open(original)
            .map_err(err)?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(err)?;
        if bytes != expected.as_bytes() {
            return Err("Cloud shortcut contents do not match its validated web URL.".into());
        }
        Ok(None)
    } else {
        Ok(None)
    }
}

pub fn start_worker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        let cloud = app.state::<Arc<Cloud>>();
        if cloud.busy.load(Ordering::Acquire) {
            continue;
        }
        if cloud.config().is_ok_and(|c| !c.options.paused)
            && auth::load(&cloud.root).is_ok_and(|a| a.is_some_and(|c| c.refresh_token.is_some()))
        {
            let _ = start(&app, "backup", true);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restore_checks_decoded_screenshot_and_regenerates_thumbnail() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let mut library = crate::library::Library::open(root.clone()).unwrap();
        let item = library
            .commit(Kind::Screen, image::DynamicImage::new_rgba8(320, 200), None)
            .unwrap();
        let mut backup = library.backup_items().unwrap().remove(0);
        let source = library.export_source(&item.id).unwrap().0;
        let thumbnail = prepare_restore(&backup, &source, &root).unwrap().unwrap();
        assert_eq!(image::open(thumbnail).unwrap().width(), 160);
        backup.item.width = 321;
        assert!(prepare_restore(&backup, &source, &root).is_err());
        let link = library
            .commit_url("https://example.com/", None, None)
            .unwrap();
        let backup = library.backup_items().unwrap().pop().unwrap();
        let source = library.export_source(&link.id).unwrap().0;
        assert!(prepare_restore(&backup, &source, &root).unwrap().is_none());
        fs::write(
            &source,
            b"[InternetShortcut]\r\nURL=file:///C:/Windows/\r\n",
        )
        .unwrap();
        assert!(prepare_restore(&backup, &source, &root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn upload_cutoff_does_not_hide_history_from_restore_and_operations_serialize() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let mut library = crate::library::Library::open(root.clone()).unwrap();
        library
            .commit_url("https://example.com/", None, None)
            .unwrap();
        let mut backup = library.backup_items().unwrap().remove(0);
        backup.item.created_at = "2026-01-01T00:00:00Z".into();
        let config = Config {
            since: Some("2026-02-01T00:00:00Z".into()),
            ..Config::default()
        };
        assert!(!selected(&backup, &config, true));
        assert!(selected(&backup, &config, false));
        let cloud = Cloud::new(root.clone());
        let guard = cloud.reserve().unwrap();
        assert!(cloud.reserve().is_err());
        drop(guard);
        assert!(cloud.reserve().is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_cloud_settings_are_preserved() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        fs::write(root.join("cloud-settings.json"), b"broken").unwrap();
        let cloud = Cloud::new(root.clone());
        assert!(cloud.config().is_err());
        assert_eq!(
            fs::read(root.join("cloud-settings.json")).unwrap(),
            b"broken"
        );
        fs::remove_file(root.join("cloud-settings.json")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
