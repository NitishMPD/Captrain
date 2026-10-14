mod capture;
mod desktop;
mod foreground;
mod library;
mod shelf_window;
mod shortcuts;
mod updates;
#[cfg(debug_assertions)]
mod verification;

use library::{err, Kind, Library, Result};
use std::sync::{atomic::AtomicBool, Arc, Condvar, Mutex};
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub struct AppState {
    library: Mutex<Library>,
    settings: Mutex<shortcuts::StoredSettings>,
    shelf: Mutex<shelf_window::Shelf>,
    timer: Condvar,
    busy: AtomicBool,
    cancelled: AtomicBool,
    recording: AtomicBool,
    pending: Mutex<Option<capture::Pending>>,
    viewer: Mutex<Option<desktop::ViewerContext>>,
}
fn shelf_only(window: &WebviewWindow) -> Result<()> {
    if window.label() == "shelf" {
        Ok(())
    } else {
        Err("This command is only available to the shelf.".into())
    }
}
fn overlay_only(window: &WebviewWindow) -> Result<()> {
    if window.label() == "overlay" {
        Ok(())
    } else {
        Err("No capture selector is active.".into())
    }
}

#[tauri::command]
fn shelf_state(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
) -> Result<shelf_window::View> {
    shelf_only(&window)?;
    Ok(state.shelf.lock().unwrap().view.clone())
}
#[tauri::command]
fn shelf_action(
    window: WebviewWindow,
    app: AppHandle,
    action: String,
    reveal: Option<u64>,
) -> Result<()> {
    shelf_only(&window)?;
    shelf_window::action_for(&app, &action, reveal)
}
#[tauri::command]
fn start_capture(
    window: WebviewWindow,
    app: AppHandle,
    kind: Kind,
    direct: Option<bool>,
) -> Result<()> {
    shelf_only(&window)?;
    capture::start_with(&app, kind, direct.unwrap_or(false))
}
#[tauri::command]
async fn list_captures(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
    offset: usize,
    limit: usize,
    folder_id: Option<String>,
    exported_only: Option<bool>,
) -> Result<library::Page> {
    desktop::content_only(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(state.library.lock().unwrap().list_filtered(
            &query,
            offset,
            limit,
            folder_id.as_deref(),
            exported_only.unwrap_or(false),
        ))
    })
    .await
    .map_err(err)?
}
#[tauri::command]
async fn capture_image(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    thumbnail: bool,
) -> Result<String> {
    desktop::content_only(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.library.lock().unwrap().image(&id, thumbnail)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
async fn copy_capture(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<()> {
    desktop::content_only(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.library.lock().unwrap().copy(&id))
        .await
        .map_err(err)?
}
#[tauri::command]
async fn delete_capture(
    window: WebviewWindow,
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<()> {
    desktop::content_only(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.library.lock().unwrap().delete(&id))
        .await
        .map_err(err)??;
    desktop::changed(&app);
    Ok(())
}
#[tauri::command]
async fn save_capture(
    window: WebviewWindow,
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<bool> {
    desktop::content_only(&window)?;
    let state = state.inner().clone();
    let (source, name) = state.library.lock().unwrap().export_source(&id)?;
    shelf_window::action(&app, "hold")?;
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Save capture")
        .set_file_name(name)
        .add_filter("PNG image", &["png"])
        .save_file()
        .await;
    let _ = shelf_window::action(&app, "release");
    let Some(file) = dialog else {
        return Ok(false);
    };
    let destination = file.path().to_path_buf();
    let root = state
        .library
        .lock()
        .unwrap()
        .root
        .canonicalize()
        .map_err(err)?;
    let parent = destination
        .parent()
        .ok_or("Invalid export location.")?
        .canonicalize()
        .map_err(err)?;
    if parent.starts_with(&root) {
        return Err("Choose an export location outside Captrain's private library.".into());
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(source).map_err(err)?;
        library::atomic_write(&destination, &bytes)?;
        state
            .library
            .lock()
            .unwrap()
            .mark_exported(&id, destination.clone())
            .map_err(|e| {
                format!(
                    "Image saved at {}, but export history could not update: {e}",
                    destination.display()
                )
            })?;
        desktop::changed(&app2);
        Ok(true)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
fn get_settings(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
) -> Result<shortcuts::Settings> {
    shelf_only(&window)?;
    Ok(state.settings.lock().unwrap().value.clone())
}
#[tauri::command]
fn update_settings(
    window: WebviewWindow,
    app: AppHandle,
    settings: shortcuts::Settings,
) -> Result<shortcuts::Settings> {
    shelf_only(&window)?;
    shortcuts::update(&app, settings)
}
#[tauri::command]
fn restore_shortcuts(
    window: WebviewWindow,
    app: AppHandle,
    state: tauri::State<Arc<AppState>>,
) -> Result<shortcuts::Settings> {
    shelf_only(&window)?;
    let mut value = state.settings.lock().unwrap().value.clone();
    value.shortcuts = shortcuts::Settings::default().shortcuts;
    shortcuts::update(&app, value)
}
#[derive(serde::Serialize)]
struct Display {
    id: String,
    name: String,
}
#[tauri::command]
fn list_displays(window: WebviewWindow) -> Result<Vec<Display>> {
    shelf_only(&window)?;
    shelf_window::displays()?
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            Ok(Display {
                name: format!(
                    "Display {} · {} × {}",
                    i + 1,
                    d.bounds.right - d.bounds.left,
                    d.bounds.bottom - d.bounds.top
                ),
                id: d.id,
            })
        })
        .collect()
}
#[tauri::command]
fn overlay_data(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
) -> Result<capture::Overlay> {
    overlay_only(&window)?;
    state
        .pending
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.data.clone())
        .ok_or("Capture is no longer active.".into())
}
#[tauri::command]
fn finish_selection(
    window: WebviewWindow,
    app: AppHandle,
    selection: capture::Selection,
) -> Result<()> {
    overlay_only(&window)?;
    capture::select(&app, selection)
}
#[tauri::command]
fn cancel_capture(window: WebviewWindow, app: AppHandle) -> Result<()> {
    overlay_only(&window)?;
    capture::cancel(&app);
    Ok(())
}
#[tauri::command]
fn record_shortcuts(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    active: bool,
) -> Result<()> {
    shelf_only(&window)?;
    state
        .recording
        .store(active, std::sync::atomic::Ordering::Release);
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::default())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = shelf_window::action(app, "open");
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let Some(state) = app.try_state::<Arc<AppState>>() else {
                        return;
                    };
                    if state.viewer.lock().unwrap().is_some() {
                        return;
                    }
                    let action = {
                        let settings = state.settings.lock().unwrap();
                        settings
                            .value
                            .shortcuts
                            .iter()
                            .find(|(_, s)| {
                                s.parse::<Shortcut>().is_ok_and(|s| s.id() == shortcut.id())
                            })
                            .map(|(k, _)| k.clone())
                    };
                    if state.recording.load(std::sync::atomic::Ordering::Acquire) {
                        if let Some(action) = action {
                            use tauri::Emitter;
                            let binding =
                                state.settings.lock().unwrap().value.shortcuts[&action].clone();
                            let _ = app.emit_to("shelf", "shortcut-recorded", binding);
                        }
                        return;
                    }
                    let result = match action.as_deref() {
                        Some("toggle") => shelf_window::action(app, "toggle"),
                        Some("region") => capture::start(app, Kind::Region),
                        Some("window") => {
                            let direct = state.settings.lock().unwrap().value.instant_capture;
                            capture::start_with(app, Kind::Window, direct)
                        }
                        Some("screen") => capture::start(app, Kind::Screen),
                        _ => Ok(()),
                    };
                    if let Err(e) = result {
                        let mut shelf = state.shelf.lock().unwrap();
                        if shelf.view.mode == shelf_window::Mode::Collapsed {
                            shelf.begin();
                            shelf.finish(false, Some(e), std::time::Instant::now());
                        } else {
                            shelf.view.error = Some(e);
                        }
                        drop(shelf);
                        shelf_window::apply(app, false);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            updates::check_update,
            updates::download_update,
            updates::install_update,
            shelf_state,
            shelf_action,
            start_capture,
            list_captures,
            capture_image,
            copy_capture,
            save_capture,
            delete_capture,
            get_settings,
            update_settings,
            restore_shortcuts,
            list_displays,
            overlay_data,
            finish_selection,
            cancel_capture,
            record_shortcuts,
            desktop::list_folders,
            desktop::create_folder,
            desktop::rename_folder,
            desktop::delete_folder,
            desktop::move_capture,
            desktop::choose_capture_folder,
            desktop::open_capture_folder,
            desktop::storage_path,
            desktop::open_viewer,
            desktop::viewer_context,
            desktop::close_viewer
        ])
        .setup(|app| {
            let root = {
                #[cfg(debug_assertions)]
                {
                    if std::env::var_os("CAPTRAIN_VERIFY").is_some() {
                        std::env::temp_dir()
                            .join(format!("captrain-verification-{}", uuid::Uuid::new_v4()))
                    } else {
                        app.path().app_local_data_dir()?
                    }
                }
                #[cfg(not(debug_assertions))]
                {
                    app.path().app_local_data_dir()?
                }
            };
            let library = Library::open(root.clone()).map_err(std::io::Error::other)?;
            let settings = shortcuts::load(&root).map_err(std::io::Error::other)?;
            let shortcut_warning = shortcuts::register_initial(app.handle(), &settings.value);
            let warning = [
                library.warning.clone(),
                settings.warning.clone(),
                shortcut_warning,
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
            let state = Arc::new(AppState {
                library: Mutex::new(library),
                settings: Mutex::new(settings),
                shelf: Mutex::new(shelf_window::Shelf::new(if warning.is_empty() {
                    None
                } else {
                    Some(warning)
                })),
                timer: Condvar::new(),
                busy: AtomicBool::new(false),
                cancelled: AtomicBool::new(false),
                recording: AtomicBool::new(false),
                pending: Mutex::new(None),
                viewer: Mutex::new(None),
            });
            app.manage(state.clone());
            foreground::start().map_err(std::io::Error::other)?;
            shelf_window::watch_displays(app.handle()).map_err(std::io::Error::other)?;
            shelf_window::timer(app.handle().clone(), state);
            let open =
                tauri::menu::MenuItem::with_id(app, "open", "Open shelf", true, None::<&str>)?;
            let exit =
                tauri::menu::MenuItem::with_id(app, "exit", "Exit Captrain", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&open, &exit])?;
            let pixels = image::load_from_memory(include_bytes!("../icons/32x32.png"))?.into_rgba8();
            let icon = tauri::image::Image::new_owned(pixels.into_raw(), 32, 32);
            tauri::tray::TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Captrain")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        let _ = shelf_window::action(app, "open");
                    }
                    "exit" => {
                        let _ = app.global_shortcut().unregister_all();
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        }
                    ) {
                        let _ = shelf_window::action(tray.app_handle(), "open");
                    }
                })
                .build(app)?;
            #[cfg(debug_assertions)]
            verification::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "viewer" && matches!(event, tauri::WindowEvent::Destroyed) {
                desktop::viewer_closed(window.app_handle());
            }
            if window.label() == "shelf" {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = shelf_window::action(window.app_handle(), "collapse");
                    }
                    tauri::WindowEvent::Focused(false) => {
                        let state = window.state::<Arc<AppState>>();
                        state
                            .recording
                            .store(false, std::sync::atomic::Ordering::Release);
                        let manual =
                            state.shelf.lock().unwrap().view.mode == shelf_window::Mode::Manual;
                        let mut pid = 0;
                        unsafe {
                            use windows::Win32::UI::WindowsAndMessaging::{
                                GetForegroundWindow, GetWindowThreadProcessId,
                            };
                            GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
                        }
                        if manual
                            && pid != std::process::id()
                            && state.viewer.lock().unwrap().is_none()
                        {
                            let _ = shelf_window::action(window.app_handle(), "collapse");
                        }
                    }
                    tauri::WindowEvent::ScaleFactorChanged { .. } => {
                        shelf_window::apply(window.app_handle(), false)
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Captrain could not start");
}
