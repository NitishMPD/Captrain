use crate::{
    library::{err, Result},
    shelf_only, AppState,
};
use std::sync::{atomic::Ordering, Arc};
use tauri::{ipc::Channel, AppHandle, Manager, WebviewWindow};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct Updates(tauri::async_runtime::Mutex<Pending>);

#[derive(Default)]
struct Pending {
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}

#[derive(serde::Serialize)]
pub struct Available {
    version: String,
    notes: Option<String>,
}

#[tauri::command]
pub async fn check_update(
    window: WebviewWindow,
    app: AppHandle,
    updates: tauri::State<'_, Updates>,
) -> Result<Option<Available>> {
    shelf_only(&window)?;
    if cfg!(debug_assertions) {
        return Err("Updates are available in the installed Windows x64 app.".into());
    }
    let mut pending = updates.0.try_lock().map_err(err)?;
    if pending.bytes.is_some() {
        return Err("An update is already downloaded.".into());
    }
    let update = app
        .updater_builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(err)?
        .check()
        .await
        .map_err(err)?;
    let available = update.as_ref().map(|u| Available {
        version: u.version.clone(),
        notes: u.body.clone(),
    });
    pending.update = update;
    Ok(available)
}

#[tauri::command]
pub async fn download_update(
    window: WebviewWindow,
    updates: tauri::State<'_, Updates>,
    progress: Channel<Option<f64>>,
) -> Result<()> {
    shelf_only(&window)?;
    let mut pending = updates.0.try_lock().map_err(err)?;
    let update = pending.update.as_ref().ok_or("Check for updates first.")?;
    let mut downloaded = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let percent = total
                    .filter(|n| *n > 0)
                    .map(|n| (downloaded as f64 / n as f64 * 100.0).min(100.0));
                let _ = progress.send(percent);
            },
            || {},
        )
        .await
        .map_err(err)?;
    pending.bytes = Some(bytes); // The updater verifies the signature before returning bytes.
    Ok(())
}

#[tauri::command]
pub async fn install_update(
    window: WebviewWindow,
    app: AppHandle,
    updates: tauri::State<'_, Updates>,
) -> Result<()> {
    shelf_only(&window)?;
    let pending = updates.0.try_lock().map_err(err)?;
    let update = pending.update.as_ref().ok_or("Check for updates first.")?;
    let bytes = pending.bytes.as_ref().ok_or("Download the update first.")?;
    let state = app.state::<Arc<AppState>>();
    // Share capture/viewer exclusion so acquisition or selection cannot race installation.
    state
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Finish the capture or recording, or close the viewer before restarting.".to_string())?;
    // Wait for any active synchronous persistence operation before the installer exits the app.
    let _settings = state.settings.lock().unwrap();
    let _library = state.library.lock().unwrap();
    let result = update.install(bytes).map_err(err);
    state.busy.store(false, Ordering::Release);
    result
}
