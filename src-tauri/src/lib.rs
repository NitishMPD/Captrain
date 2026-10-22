mod capture;
mod cloud;
mod desktop;
mod foreground;
mod library;
mod links;
mod recording;
mod shelf_window;
mod shortcuts;
mod startup;
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
    video: Mutex<recording::Session>,
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
    images_only: Option<bool>,
    captures_only: Option<bool>,
) -> Result<library::Page> {
    desktop::content_only(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(state.library.lock().unwrap().list_content(
            &query,
            offset,
            limit,
            folder_id.as_deref(),
            exported_only.unwrap_or(false),
            if images_only.unwrap_or(false) {
                library::Content::Images
            } else if captures_only.unwrap_or(false) {
                library::Content::Captures
            } else {
                library::Content::All
            },
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
async fn save_selected(
    window: WebviewWindow,
    app: AppHandle,
    ids: Vec<String>,
) -> Result<Option<library::BatchResult>> {
    shelf_only(&window)?;
    let state = app.state::<Arc<AppState>>().inner().clone();
    {
        let library = state.library.lock().unwrap();
        for id in &ids {
            library.export_source(id)?;
        }
    }
    if ids.is_empty() {
        return Ok(None);
    }
    shelf_window::action(&app, "hold")?;
    let picked = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Save selected items to folder")
        .pick_folder()
        .await;
    let result = if let Some(picked) = picked {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let result = state
                .library
                .lock()
                .unwrap()
                .save_unexported(&ids, picked.path());
            desktop::changed(&app);
            result.map(Some)
        })
        .await
        .map_err(err)
        .and_then(|result| result)
    } else {
        Ok(None)
    };
    let _ = shelf_window::action(&app, "release");
    result
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
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Capture has no extension.")?
        .to_owned();
    let label = match extension.as_str() {
        "url" => "Internet shortcut",
        "mp4" => "MP4 video",
        _ => "PNG image",
    };
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Save capture")
        .set_file_name(name)
        .add_filter(label, &[extension.as_str()])
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
        library::atomic_copy(&source, &destination)?;
        state
            .library
            .lock()
            .unwrap()
            .mark_exported(&id, destination.clone())
            .map_err(|e| {
                format!(
                    "Saved at {}, but export history could not update: {e}",
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
    desktop::content_only(&window)?;
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
    action: Option<String>,
) -> Result<shortcuts::Settings> {
    shelf_only(&window)?;
    let mut value = state.settings.lock().unwrap().value.clone();
    let defaults = shortcuts::Settings::default().shortcuts;
    match action {
        Some(id) => match defaults.get(&id) {
            Some(default) => {
                value.shortcuts.insert(id, default.clone());
            }
            None => return Err("Unknown shortcut.".into()),
        },
        None => value.shortcuts = defaults,
    }
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
                        Some("url") => links::start(app),
                        Some("video") => recording::open_setup(app, true),
                        _ => Ok(()),
                    };
                    if let Err(e) = result {
                        let mut shelf = state.shelf.lock().unwrap();
                        if shelf.view.mode == shelf_window::Mode::Collapsed {
                            shelf.begin();
                            shelf.finish(false, Some(e), std::time::Instant::now(), false);
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
            cloud::cloud_status,
            cloud::cloud_configure,
            cloud::cloud_options,
            cloud::cloud_action,
            updates::check_update,
            updates::download_update,
            updates::install_update,
            shelf_state,
            shelf_action,
            shelf_window::drag_shelf,
            start_capture,
            recording::recording_preferences,
            recording::countdown_ready,
            recording::recording_targets,
            recording::recording_devices,
            recording::recording_state,
            recording::start_recording,
            recording::recording_control,
            recording::play_recording,
            recording::island_layout,
            recording::island_move,
            recording::island_drag,
            recording::close_island,
            recording::open_recording,
            recording::island_window,
            links::save_link,
            links::open_link,
            links::rename_link,
            startup::get_startup,
            startup::set_startup,
            list_captures,
            capture_image,
            copy_capture,
            desktop::copy_path,
            save_capture,
            save_selected,
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
            desktop::create_permanent_folder,
            desktop::rename_folder,
            desktop::delete_folder,
            desktop::move_capture,
            desktop::choose_capture_folder,
            desktop::open_capture_folder,
            desktop::reveal_capture,
            desktop::open_folder,
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
                video: Mutex::new(recording::Session::default()),
                pending: Mutex::new(None),
                viewer: Mutex::new(None),
            });
            app.manage(state.clone());
            app.manage(Arc::new(cloud::Cloud::new(root)));
            cloud::start_worker(app.handle().clone());
            foreground::start().map_err(std::io::Error::other)?;
            shelf_window::watch_displays(app.handle()).map_err(std::io::Error::other)?;
            shelf_window::timer(app.handle().clone(), state);
            let open =
                tauri::menu::MenuItem::with_id(app, "open", "Open shelf", true, None::<&str>)?;
            let exit =
                tauri::menu::MenuItem::with_id(app, "exit", "Exit Captrain", true, None::<&str>)?;
            let shortcuts_only = tauri::menu::CheckMenuItem::with_id(
                app, "shortcuts-only", "Shortcuts only (hide edge bar)", true,
                app.state::<Arc<AppState>>().settings.lock().unwrap().value.shortcuts_only,
                None::<&str>,
            )?;
            app.manage(shortcuts_only.clone());
            let menu = tauri::menu::Menu::with_items(app, &[&open, &shortcuts_only, &exit])?;
            let pixels =
                image::load_from_memory(include_bytes!("../icons/32x32.png"))?.into_rgba8();
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
                    "shortcuts-only" => {
                        let state = app.state::<Arc<AppState>>();
                        let mut next = state.settings.lock().unwrap().value.clone();
                        next.shortcuts_only = !next.shortcuts_only;
                        if let Err(error) = shortcuts::update(app, next) {
                            let checked = state.settings.lock().unwrap().value.shortcuts_only;
                            let _ = app.state::<tauri::menu::CheckMenuItem<tauri::Wry>>().set_checked(checked);
                            state.shelf.lock().unwrap().view.error = Some(error);
                            let _ = shelf_window::action(app, "open");
                        }
                    }
                    "exit" => {
                        let state = app.state::<Arc<AppState>>();
                        if state.busy.load(std::sync::atomic::Ordering::Acquire) {
                            let mut shelf = state.shelf.lock().unwrap();
                            shelf.view.error = Some("Finish the current capture, recording, viewer or update before exiting.".into());
                            drop(shelf);
                            let _ = shelf_window::action(app, "open");
                        } else {
                            let _ = app.global_shortcut().unregister_all();
                            app.exit(0);
                        }
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
            let verifying = std::env::var_os("CAPTRAIN_VERIFY").is_some();
            #[cfg(not(debug_assertions))]
            let verifying = false;
            if !verifying {
                // Click-through logo splash; no IPC, so it needs no capability and just closes itself.
                let splash = tauri::WebviewWindowBuilder::new(
                    app,
                    "splash",
                    tauri::WebviewUrl::App("index.html?splash=1".into()),
                )
                .title("Captrain")
                .inner_size(240.0, 240.0)
                .center()
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .build()?;
                splash.set_ignore_cursor_events(true)?;
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                    let _ = splash.destroy();
                });
            }
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
