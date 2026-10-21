use crate::{
    library::{err, Folder, Result},
    shelf_window, shortcuts, AppState,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub fn content_only(window: &WebviewWindow) -> Result<()> {
    if matches!(window.label(), "shelf" | "viewer") {
        Ok(())
    } else {
        Err("This command is unavailable to the capture selector.".into())
    }
}
pub fn changed(app: &AppHandle) {
    let _ = app.emit("library-changed", ());
}

#[tauri::command]
pub fn list_folders(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
) -> Result<Vec<Folder>> {
    content_only(&window)?;
    Ok(state.library.lock().unwrap().folders())
}
#[tauri::command]
pub fn create_folder(window: WebviewWindow, app: AppHandle, name: String) -> Result<Folder> {
    content_only(&window)?;
    let folder = app
        .state::<Arc<AppState>>()
        .library
        .lock()
        .unwrap()
        .create_folder(&name)?;
    changed(&app);
    Ok(folder)
}
#[tauri::command]
pub fn rename_folder(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    name: String,
) -> Result<()> {
    content_only(&window)?;
    app.state::<Arc<AppState>>()
        .library
        .lock()
        .unwrap()
        .rename_folder(&id, &name)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
pub fn delete_folder(window: WebviewWindow, app: AppHandle, id: String) -> Result<()> {
    content_only(&window)?;
    let state = app.state::<Arc<AppState>>();
    // Reset the destination first; a failed folder deletion only leaves new captures unfiled.
    let mut settings = state.settings.lock().unwrap().value.clone();
    if settings.folder_ids().any(|f| f == id) {
        settings.forget_folder(&id);
        shortcuts::update(&app, settings)?;
    }
    state.library.lock().unwrap().delete_folder(&id)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
pub fn move_capture(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    folder_id: Option<String>,
) -> Result<()> {
    content_only(&window)?;
    let state = app.state::<Arc<AppState>>();
    let mut library = state.library.lock().unwrap();
    library.move_capture(&id, folder_id.as_deref())?;
    // Filing into a permanent folder also saves the file there (once).
    let item = library.get(&id).cloned();
    if let (Some(item), Some(dir)) = (item, library.export_dir(folder_id.as_deref(), None)) {
        if !item.exported_path.as_deref().is_some_and(|p| p.starts_with(&dir)) {
            if let Err(e) = library.auto_export(&id, &dir) {
                drop(library);
                changed(&app);
                return Err(format!("Moved, but saving to the PC folder failed: {e}"));
            }
        }
    }
    drop(library);
    changed(&app);
    Ok(())
}
#[tauri::command]
pub async fn create_permanent_folder(window: WebviewWindow, app: AppHandle) -> Result<Option<Folder>> {
    crate::shelf_only(&window)?;
    shelf_window::action(&app, "hold")?;
    let picked = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Choose a folder on this PC")
        .pick_folder()
        .await;
    let _ = shelf_window::action(&app, "release");
    let Some(picked) = picked else { return Ok(None) };
    let folder = app
        .state::<Arc<AppState>>()
        .library
        .lock()
        .unwrap()
        .create_permanent_folder(picked.path())?;
    changed(&app);
    Ok(Some(folder))
}
#[tauri::command]
pub async fn choose_capture_folder(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<shortcuts::Settings> {
    crate::shelf_only(&window)?;
    shelf_window::action(&app, "hold")?;
    let folder = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Automatically save new captures in…")
        .pick_folder()
        .await;
    let _ = shelf_window::action(&app, "release");
    let mut settings = app
        .state::<Arc<AppState>>()
        .settings
        .lock()
        .unwrap()
        .value
        .clone();
    if let Some(folder) = folder {
        settings.capture_folder = Some(folder.path().to_path_buf());
        shortcuts::update(&app, settings)
    } else {
        Ok(settings)
    }
}
#[tauri::command]
pub fn storage_path(window: WebviewWindow, state: tauri::State<Arc<AppState>>) -> Result<String> {
    content_only(&window)?;
    Ok(state
        .library
        .lock()
        .unwrap()
        .root
        .join("captures")
        .display()
        .to_string())
}
#[tauri::command]
pub fn open_capture_folder(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
) -> Result<()> {
    content_only(&window)?;
    let directory = state
        .settings
        .lock()
        .unwrap()
        .value
        .capture_folder
        .clone()
        .unwrap_or_else(|| state.library.lock().unwrap().root.join("captures"));
    if !directory.is_dir() {
        return Err("The save folder is unavailable.".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(directory)
        .spawn()
        .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn reveal_capture(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    id: String,
) -> Result<()> {
    content_only(&window)?;
    let path = state.library.lock().unwrap().capture_path(&id)?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
pub fn copy_path(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    id: String,
    folder: bool,
) -> Result<()> {
    content_only(&window)?;
    let library = state.library.lock().unwrap();
    let path = if folder {
        library
            .folders()
            .into_iter()
            .find(|f| f.id == id)
            .and_then(|f| f.path)
            .ok_or("This folder is not on your PC.")?
    } else {
        library.capture_path(&id)?
    };
    arboard::Clipboard::new()
        .map_err(err)?
        .set_text(path.to_string_lossy().into_owned())
        .map_err(err)
}

#[tauri::command]
pub fn open_folder(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    id: String,
) -> Result<()> {
    content_only(&window)?;
    let directory = state
        .library
        .lock()
        .unwrap()
        .folders()
        .into_iter()
        .find(|f| f.id == id)
        .and_then(|f| f.path)
        .ok_or("This folder is not on your PC.")?;
    if !directory.is_dir() {
        return Err("The folder is unavailable.".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(directory)
        .spawn()
        .map_err(err)?;
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerContext {
    pub id: String,
    pub query: String,
    pub folder_id: Option<String>,
    pub exported_only: bool,
}

#[tauri::command]
pub async fn open_viewer(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    query: String,
    folder_id: Option<String>,
    exported_only: bool,
) -> Result<()> {
    crate::shelf_only(&window)?;
    let state = app.state::<Arc<AppState>>();
    let non_image = {
        let library = state.library.lock().unwrap();
        library.url(&id).is_ok() || library.is_video(&id)?
    };
    if non_image {
        return Err("Open links and recordings with their corresponding player.".into());
    }
    state.library.lock().unwrap().export_source(&id)?;
    if let Some(viewer) = app.get_webview_window("viewer") {
        viewer.set_focus().map_err(err)?;
        return Ok(());
    }
    let context = ViewerContext {
        id,
        query,
        folder_id,
        exported_only,
    };
    let monitor = window.current_monitor().map_err(err)?;
    let (width, height) = monitor
        .map(|m| {
            let size = &m.work_area().size;
            let scale = m.scale_factor();
            (
                (size.width as f64 / scale - 40.0).clamp(1.0, 1120.0),
                (size.height as f64 / scale - 80.0).clamp(1.0, 800.0),
            )
        })
        .unwrap_or((1000.0, 720.0));
    state
        .busy
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .map_err(|_| {
            "Finish the current capture or viewer before opening another image.".to_string()
        })?;
    *state.viewer.lock().unwrap() = Some(context);
    if let Err(error) = shelf_window::action(&app, "hold") {
        viewer_closed(&app);
        return Err(error);
    }
    let built = WebviewWindowBuilder::new(
        &app,
        "viewer",
        WebviewUrl::App("index.html?viewer=1".into()),
    )
    .title("Captrain — Image viewer")
    .inner_size(width, height)
    .min_inner_size(width.min(320.0), height.min(300.0))
    .center()
    .always_on_top(true)
    .visible(false)
    .owner(&window)
    .map_err(err)
    .and_then(|builder| builder.build().map_err(err));
    let viewer = match built {
        Ok(viewer) => viewer,
        Err(error) => {
            viewer_closed(&app);
            return Err(error);
        }
    };
    if let Err(error) = window
        .set_enabled(false)
        .and_then(|_| viewer.show())
        .and_then(|_| viewer.set_focus())
    {
        let _ = viewer.destroy();
        viewer_closed(&app);
        return Err(err(error));
    }
    Ok(())
}
#[tauri::command]
pub fn viewer_context(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
) -> Result<ViewerContext> {
    if window.label() != "viewer" {
        return Err("No image viewer is open.".into());
    }
    state
        .viewer
        .lock()
        .unwrap()
        .clone()
        .ok_or("No image viewer is open.".into())
}
#[tauri::command]
pub fn close_viewer(window: WebviewWindow) -> Result<()> {
    if window.label() != "viewer" {
        return Err("No image viewer is open.".into());
    }
    window.close().map_err(err)
}
pub fn viewer_closed(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>();
    if state.viewer.lock().unwrap().take().is_none() {
        return;
    }
    state
        .busy
        .store(false, std::sync::atomic::Ordering::Release);
    if let Some(shelf) = app.get_webview_window("shelf") {
        let _ = shelf.set_enabled(true);
    }
    let _ = shelf_window::action(app, "release");
    let _ = shelf_window::action(app, "open");
    changed(app);
}
