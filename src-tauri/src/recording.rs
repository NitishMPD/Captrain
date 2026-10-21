use crate::{
    capture,
    library::{err, Kind, Result},
    shelf_window, AppState,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{atomic::Ordering, mpsc, Arc},
    time::{Duration, Instant},
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::HWND,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
};

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    pub kind: Kind,
    pub display: String,
    pub window: Option<String>,
    pub fps: u32,
    pub quality: String,
    pub resolution: u32,
    pub cursor: bool,
    pub countdown: bool,
    pub system_audio: bool,
    pub microphone: Option<String>,
    pub camera: Option<String>,
    pub corner: String,
    pub camera_size: u32,
}
impl Options {
    fn validate(&self) -> Result<()> {
        if self.display.len() > 4096
            || self.window.as_ref().is_some_and(|w| w.len() > 64)
            || !matches!(self.kind, Kind::Screen | Kind::Window | Kind::Region)
            || ![30, 60].contains(&self.fps)
            || ![0, 720, 1080].contains(&self.resolution)
            || !["standard", "high", "maximum"].contains(&self.quality.as_str())
            || !["TopLeft", "TopRight", "BottomLeft", "BottomRight"].contains(&self.corner.as_str())
            || ![15, 22, 30].contains(&self.camera_size)
            || [&self.microphone, &self.camera]
                .iter()
                .any(|s| s.as_ref().is_some_and(|s| s.is_empty() || s.len() > 4096))
        {
            return Err("Invalid recording options.".into());
        }
        Ok(())
    }
}

fn preferences(path: &std::path::Path, options: Option<Options>) -> Result<Option<Options>> {
    if let Some(mut options) = options {
        options.validate()?;
        // Native window handles are transient and may belong to another app after restart.
        options.window = None;
        crate::library::atomic_write(path, &serde_json::to_vec(&options).map_err(err)?)?;
        return Ok(Some(options));
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let options: Options = serde_json::from_slice(&bytes).map_err(err)?;
            options.validate()?;
            Ok(Some(options))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}

#[tauri::command]
pub fn recording_preferences(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    options: Option<Options>,
) -> Result<Option<Options>> {
    crate::shelf_only(&window)?;
    let library = state.library.lock().unwrap();
    preferences(&library.root.join("recording-settings.json"), options)
}

#[tauri::command]
pub fn countdown_ready(window: WebviewWindow, state: tauri::State<Arc<AppState>>) -> Result<()> {
    if window.label() != "recording-countdown" {
        return Err("No recording countdown is active.".into());
    }
    state
        .video
        .lock()
        .unwrap()
        .control
        .as_ref()
        .ok_or("No recording is active.")?
        .try_send("ready".into())
        .map_err(err)
}

fn countdown_window(
    app: &AppHandle,
    bounds: windows::Win32::Foundation::RECT,
    label: &str,
) -> Result<()> {
    let app2 = app.clone();
    let mut url = tauri::Url::parse("https://captrain.invalid/index.html").map_err(err)?;
    url.query_pairs_mut()
        .append_pair("countdown", "1")
        .append_pair("target", label);
    let route = format!("index.html?{}", url.query().unwrap());
    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result: Result<()> = (|| {
            let window = WebviewWindowBuilder::new(
                &app2,
                "recording-countdown",
                WebviewUrl::App(route.into()),
            )
            .title("Captrain recording countdown")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build()
            .map_err(err)?;
            window.set_content_protected(true).map_err(err)?;
            window
                .set_position(PhysicalPosition::new(bounds.left, bounds.top))
                .map_err(err)?;
            window
                .set_size(PhysicalSize::new(
                    (bounds.right - bounds.left) as u32,
                    (bounds.bottom - bounds.top) as u32,
                ))
                .map_err(err)?;
            window.show().map_err(err)?;
            window.set_focus().map_err(err)?;
            Ok(())
        })();
        let _ = tx.send(result);
    })
    .map_err(err)?;
    rx.recv_timeout(Duration::from_secs(10)).map_err(err)?
}

fn dimensions(w: u32, h: u32, limit: u32) -> Result<(u32, u32)> {
    if w < 2 || h < 2 {
        return Err("Choose an area at least 2 × 2 pixels.".into());
    }
    let scale = if limit == 0 {
        1.0
    } else {
        (limit as f64 / h as f64).min(1.0)
    };
    let (w, h) = (
        ((w as f64 * scale) as u32) & !1,
        ((h as f64 * scale) as u32) & !1,
    );
    if w < 2 || h < 2 {
        return Err("The scaled recording is too narrow.".into());
    }
    Ok((w, h))
}
fn bitrate(w: u32, h: u32, fps: u32, quality: &str) -> u32 {
    let factor = match quality {
        "standard" => 10,
        "maximum" => 24,
        _ => 16,
    };
    (w as u64 * h as u64 * fps as u64 * factor / 100).clamp(2_000_000, 120_000_000) as u32
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub status: String,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub frames: u64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub options: Option<Options>,
}
pub struct Session {
    pub view: View,
    control: Option<mpsc::SyncSender<String>>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            view: View {
                status: "idle".into(),
                ..Default::default()
            },
            control: None,
        }
    }
}
pub fn open_setup(app: &AppHandle) -> Result<()> {
    let state = app.state::<Arc<AppState>>();
    let active = state.video.lock().unwrap().view.status != "idle";
    if state.busy.load(Ordering::Acquire) && !active {
        return Err("Finish the current operation before opening video setup.".into());
    }
    if active {
        // Same shortcut toggles: pressing it mid-recording stops it. A full queue means a stop is already pending.
        let session = state.video.lock().unwrap();
        if let Some(control) = session.control.as_ref() {
            let _ = control.try_send("stop".into());
        }
        return Ok(());
    }
    shelf_window::action(app, "open")?;
    app.emit_to("shelf", "open-recording", ()).map_err(err)?;
    Ok(())
}
fn publish(app: &AppHandle, state: &AppState, view: View) {
    state.video.lock().unwrap().view = view.clone();
    let _ = app.emit_to("shelf", "recording-state", &view);
    let _ = app.emit_to("recording-countdown", "recording-state", &view);
}
fn helper(app: &AppHandle) -> Result<Command> {
    let path = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("recorder/Captrain.Recorder.exe")
    } else {
        app.path()
            .resource_dir()
            .map_err(err)?
            .join("recorder/Captrain.Recorder.exe")
    };
    if !path.is_file() {
        return Err(if cfg!(debug_assertions) {
            "The recording component is missing. Run npm run recorder:build, then restart Captrain."
        } else {
            "The recording component is missing. Reinstall Captrain to restore it."
        }
        .into());
    }
    let mut command = Command::new(&path);
    command
        .current_dir(path.parent().unwrap())
        .creation_flags(0x08000000)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    Ok(command)
}

#[derive(Serialize)]
pub struct Target {
    id: String,
    name: String,
}
#[derive(Serialize)]
pub struct Targets {
    displays: Vec<Target>,
    windows: Vec<Target>,
}
#[tauri::command]
pub fn recording_targets(window: WebviewWindow) -> Result<Targets> {
    crate::shelf_only(&window)?;
    let displays = shelf_window::displays()?
        .into_iter()
        .enumerate()
        .map(|(i, d)| Target {
            id: d.id,
            name: format!(
                "Display {} · {} × {}",
                i + 1,
                d.bounds.right - d.bounds.left,
                d.bounds.bottom - d.bounds.top
            ),
        })
        .collect();
    let windows = windows_capture::window::Window::enumerate()
        .map_err(err)?
        .into_iter()
        .filter(|w| capture::eligible(HWND(w.as_raw_hwnd())).is_ok())
        .filter_map(|w| {
            w.title().ok().map(|name| Target {
                id: (w.as_raw_hwnd() as isize).to_string(),
                name,
            })
        })
        .collect();
    Ok(Targets { displays, windows })
}
#[tauri::command]
pub async fn recording_devices(window: WebviewWindow, app: AppHandle) -> Result<serde_json::Value> {
    crate::shelf_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = helper(&app)?.spawn().map_err(err)?;
        writeln!(child.stdin.take().unwrap(), "{{\"operation\":\"devices\"}}").map_err(err)?;
        let started = Instant::now();
        loop {
            if child.try_wait().map_err(err)?.is_some() {
                break;
            }
            if started.elapsed() > Duration::from_secs(15) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Device discovery timed out.".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let output = child.wait_with_output().map_err(err)?;
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(err)?;
        if let Some(e) = value.get("error").and_then(|v| v.as_str()) {
            return Err(e.to_string());
        }
        Ok(value)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub fn recording_state(window: WebviewWindow, state: tauri::State<Arc<AppState>>) -> Result<View> {
    if window.label() != "recording-countdown" {
        crate::shelf_only(&window)?;
    }
    Ok(state.video.lock().unwrap().view.clone())
}
#[tauri::command]
pub fn recording_control(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    action: String,
) -> Result<()> {
    if window.label() == "recording-countdown" {
        if action != "stop" {
            return Err("Only cancel is available during countdown.".into());
        }
    } else {
        crate::shelf_only(&window)?;
    }
    if !["stop", "pause", "resume"].contains(&action.as_str()) {
        return Err("Unknown recording action.".into());
    }
    let session = state.video.lock().unwrap();
    session
        .control
        .as_ref()
        .ok_or("No recording is active.")?
        .try_send(action)
        .map_err(err)
}
#[tauri::command]
pub fn start_recording(window: WebviewWindow, app: AppHandle, options: Options) -> Result<()> {
    crate::shelf_only(&window)?;
    options.validate()?;
    helper(&app)?;
    let state = app.state::<Arc<AppState>>().inner().clone();
    let display = shelf_window::displays()?
        .into_iter()
        .find(|d| d.id == options.display)
        .ok_or("Choose an available display.")?;
    let target = if options.kind == Kind::Window {
        let id: isize = options
            .window
            .as_deref()
            .ok_or("Choose a window.")?
            .parse()
            .map_err(err)?;
        capture::eligible(HWND(id as *mut _))?;
        Some(id)
    } else {
        None
    };
    {
        let settings = state.settings.lock().unwrap();
        let library = state.library.lock().unwrap();
        library.validate_recording(settings.value.folder_for(Kind::Video))?;
    }
    state
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Finish the current capture, viewer or update before recording.")?;
    let (tx, rx) = mpsc::sync_channel(4);
    state.video.lock().unwrap().control = Some(tx);
    state.cancelled.store(false, Ordering::Release);
    publish(
        &app,
        &state,
        View {
            status: "starting".into(),
            options: Some(options.clone()),
            ..Default::default()
        },
    );
    state.shelf.lock().unwrap().begin();
    state.timer.notify_one();
    std::thread::spawn(move || {
        let mut path = PathBuf::new();
        let result: Result<Option<crate::library::Item>> = (|| {
            let (x, y, w, h) = if options.kind == Kind::Region {
                let Some(rect) = capture::select_region(&app, &state, &display)? else {
                    return Ok(None);
                };
                rect
            } else if let Some(id) = target {
                let r = capture::eligible(HWND(id as *mut _))?;
                (0, 0, (r.right - r.left) as u32, (r.bottom - r.top) as u32)
            } else {
                (
                    0,
                    0,
                    (display.bounds.right - display.bounds.left) as u32,
                    (display.bounds.bottom - display.bounds.top) as u32,
                )
            };
            let (ow, oh) = dimensions(w, h, options.resolution)?;
            if options.camera.is_some() && ow.min(oh) < 128 {
                return Err(
                    "Choose an area at least 128 pixels across for the webcam inset.".into(),
                );
            }
            capture::hide_and_flush(&app)?;
            if let Some(window) = app.get_webview_window("shelf") {
                window.set_content_protected(true).map_err(err)?;
            }
            // Keep a visible, capture-excluded shelf bar available to reopen the controls.
            {
                let mut shelf = state.shelf.lock().unwrap();
                shelf.finish(false, None, Instant::now(), false);
                shelf.action("collapse", Instant::now())?;
            }
            shelf_window::apply(&app, false);
            if options.countdown {
                let displays = shelf_window::displays()?;
                let target_monitor = if let Some(id) = target {
                    let rect = capture::eligible(HWND(id as *mut _))?;
                    Some(unsafe {
                        windows::Win32::Graphics::Gdi::MonitorFromRect(
                            &rect,
                            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
                        )
                    })
                } else {
                    None
                };
                let (index, countdown_display) = displays
                    .iter()
                    .enumerate()
                    .find(|(_, d)| {
                        target_monitor.map_or(d.id == display.id, |monitor| {
                            d.monitor.as_raw_hmonitor() == monitor.0
                        })
                    })
                    .ok_or("The recording display is no longer available.")?;
                // Keep Cancel reachable even for a tiny selected region or window.
                let bounds = countdown_display.bounds;
                let label = if let Some(id) = target {
                    let window = windows_capture::window::Window::from_raw_hwnd(id as *mut _);
                    format!(
                        "Display {} - Window: {}",
                        index + 1,
                        window.title().map_err(err)?
                    )
                } else {
                    format!(
                        "Display {}{} - {w} x {h}",
                        index + 1,
                        if options.kind == Kind::Region {
                            " - Selected region"
                        } else {
                            ""
                        }
                    )
                };
                publish(
                    &app,
                    &state,
                    View {
                        status: "countdown:3".into(),
                        options: Some(options.clone()),
                        ..Default::default()
                    },
                );
                countdown_window(&app, bounds, &label)?;
                match rx.recv_timeout(Duration::from_secs(10)) {
                    Ok(action) if action == "ready" => {}
                    Ok(_) => return Ok(None),
                    Err(_) => {
                        return Err("The recording countdown could not open. Try again.".into())
                    }
                }
                for remaining in (1..=3).rev() {
                    publish(
                        &app,
                        &state,
                        View {
                            status: format!("countdown:{remaining}"),
                            options: Some(options.clone()),
                            ..Default::default()
                        },
                    );
                    if rx.recv_timeout(Duration::from_secs(1)).is_ok() {
                        return Ok(None);
                    }
                }
            }
            if rx.try_recv().is_ok() {
                return Ok(None);
            }
            if let Some(window) = app.get_webview_window("recording-countdown") {
                window.destroy().map_err(err)?;
                capture::hide_and_flush(&app)?;
                shelf_window::apply(&app, false);
            }
            let settings = state.settings.lock().unwrap().value.clone();
            let id = uuid::Uuid::new_v4().to_string();
            path = state
                .library
                .lock()
                .unwrap()
                .root
                .join("recordings")
                .join(format!("{id}.mp4"));
            let config = serde_json::json!({ "operation": "record", "path": path, "device": display.id,
                "window": target.unwrap_or(0), "x": x, "y": y, "width": w, "height": h,
                "outputWidth": ow, "outputHeight": oh, "fps": options.fps,
                "bitrate": bitrate(ow, oh, options.fps, &options.quality), "cursor": options.cursor,
                "systemAudio": options.system_audio, "microphone": options.microphone,
                "camera": options.camera, "corner": options.corner, "cameraSize": options.camera_size });
            run(&app, &state, &options, config, rx)?;
            let mut library = state.library.lock().unwrap();
            let folder = settings
                .folder_for(Kind::Video)
                .filter(|f| library.validate_folder(Some(f)).is_ok());
            let mut item = library.commit_video(&id, ow, oh, folder)?;
            let export = library.export_dir(
                item.folder_id.as_deref(),
                settings.capture_folder.as_deref(),
            );
            if let Some(directory) = export {
                match library.auto_export(&item.id, &directory) {
                    Ok(exported) => item = exported,
                    Err(e) => {
                        state.shelf.lock().unwrap().view.error = Some(format!(
                            "Recording is safe in Captrain; automatic save failed: {e}"
                        ))
                    }
                }
            }
            Ok(Some(item))
        })();
        if let Some(window) = app.get_webview_window("shelf") {
            let _ = window.set_content_protected(false);
        }
        if let Some(window) = app.get_webview_window("recording-countdown") {
            let _ = window.destroy();
        }
        state.pending.lock().unwrap().take();
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.destroy();
        }
        state.video.lock().unwrap().control = None;
        let (committed, error) = match result {
            Ok(Some(item)) => {
                let _ = app.emit("capture-created", item);
                (true, None)
            }
            Ok(None) => (false, None),
            Err(e) => (
                false,
                Some(if path.is_file() {
                    format!("{e} Partial recording retained at {}", path.display())
                } else {
                    e
                }),
            ),
        };
        publish(
            &app,
            &state,
            View {
                status: "idle".into(),
                error: error.clone(),
                ..Default::default()
            },
        );
        let shortcuts_only = state.settings.lock().unwrap().value.shortcuts_only;
        state
            .shelf
            .lock()
            .unwrap()
            .finish(committed, error, Instant::now(), shortcuts_only);
        state.busy.store(false, Ordering::Release);
        state.timer.notify_one();
        shelf_window::apply(&app, false);
    });
    Ok(())
}

fn run(
    app: &AppHandle,
    state: &AppState,
    options: &Options,
    config: serde_json::Value,
    controls: mpsc::Receiver<String>,
) -> Result<()> {
    let mut child = helper(app)?.spawn().map_err(err)?;
    let mut input = child.stdin.take().unwrap();
    if let Err(e) = writeln!(input, "{config}") {
        let _ = child.kill();
        let _ = child.wait();
        return Err(err(e));
    }
    let output = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            let event = line
                .map_err(err)
                .and_then(|s| serde_json::from_str::<View>(&s).map_err(err));
            if tx.send(event).is_err() {
                break;
            }
        }
    });
    let started = Instant::now();
    let mut active = false;
    let mut stopping = None;
    let mut last_event = Instant::now();
    let result = loop {
        if let Ok(action) = controls.try_recv() {
            if action == "stop" && stopping.is_none() {
                stopping = Some(Instant::now());
            }
            if let Err(e) = writeln!(input, "{action}") {
                break Err(err(e));
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(mut view)) => {
                last_event = Instant::now();
                if view.status == "complete" {
                    break Ok(());
                }
                if view.status == "error" {
                    break Err(view.error.unwrap_or("Recording failed.".into()));
                }
                if view.status == "idle" {
                    continue;
                }
                if view.status == "recording" {
                    active = true;
                }
                if view.status == "finishing" && stopping.is_none() {
                    stopping = Some(Instant::now());
                }
                view.options = Some(options.clone());
                publish(app, state, view);
            }
            Ok(Err(e)) => break Err(e),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err("The recorder exited before finalizing the video.".into())
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if (!active && started.elapsed() > Duration::from_secs(20))
            || stopping.is_some_and(|t| t.elapsed() > Duration::from_secs(30))
            || last_event.elapsed() > Duration::from_secs(15)
        {
            break Err("The recorder stopped responding.".into());
        }
    };
    drop(input);
    // Never leave an orphan process holding the microphone or camera.
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[tauri::command]
pub fn play_recording(
    window: WebviewWindow,
    state: tauri::State<Arc<AppState>>,
    id: String,
) -> Result<()> {
    crate::shelf_only(&window)?;
    let library = state.library.lock().unwrap();
    if !library.is_video(&id)? {
        return Err("This item is not a recording.".into());
    }
    let (path, _) = library.export_source(&id)?;
    let path = path.canonicalize().map_err(err)?;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err("Choose a default Windows app for MP4 playback.".into());
    }
    Ok(())
}
use std::os::windows::ffi::OsStrExt;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_persist_choices_without_stale_window_handles() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        let path = root.join("recording-settings.json");
        assert!(preferences(&path, None).unwrap().is_none());
        let options: Options = serde_json::from_value(serde_json::json!({ "kind":"window", "display":"display-2", "window":"123",
            "fps":30,"quality":"maximum","resolution":720,"cursor":false,"countdown":true,"systemAudio":true,
            "microphone":"mic-id","camera":"camera-id","corner":"TopLeft","cameraSize":15 })).unwrap();
        preferences(&path, Some(options.clone())).unwrap();
        let saved = preferences(&path, None).unwrap().unwrap();
        assert!(saved.window.is_none());
        let mut expected = serde_json::to_value(options.clone()).unwrap();
        expected["window"] = serde_json::Value::Null;
        assert_eq!(serde_json::to_value(saved).unwrap(), expected);
        let mut invalid = options;
        invalid.fps = 500;
        assert!(preferences(&path, Some(invalid)).is_err());
        assert_eq!(preferences(&path, None).unwrap().unwrap().fps, 30);
        std::fs::write(&path, b"broken").unwrap();
        assert!(preferences(&path, None).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn output_never_upscales_and_h264_dimensions_are_even() {
        assert_eq!(dimensions(1281, 721, 1080).unwrap(), (1280, 720));
        assert_eq!(dimensions(3840, 2160, 1080).unwrap(), (1920, 1080));
        assert_eq!(dimensions(1920, 1080, 0).unwrap(), (1920, 1080));
        assert!(dimensions(1, 100, 0).is_err());
        assert!(dimensions(2, 4000, 720).is_err());
        assert!(bitrate(1920, 1080, 60, "maximum") > bitrate(1920, 1080, 60, "high"));
    }
    #[test]
    fn options_reject_non_capture_targets_and_unbounded_values() {
        let mut o: Options = serde_json::from_value(serde_json::json!({ "kind":"screen", "display":"display", "window":null,
            "fps":60,"quality":"high","resolution":0,"cursor":true,"countdown":true,"systemAudio":false,
            "microphone":null,"camera":null,"corner":"BottomRight","cameraSize":22 })).unwrap();
        assert!(o.validate().is_ok());
        o.kind = Kind::Url;
        assert!(o.validate().is_err());
        o.kind = Kind::Screen;
        o.fps = 500;
        assert!(o.validate().is_err());
        o.fps = 30;
        o.camera_size = 500;
        assert!(o.validate().is_err());
    }
}
