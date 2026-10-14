use crate::{
    library::{err, Kind, Result},
    shelf_window::{self, DisplayInfo},
    AppState,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Dwm::{DwmFlush, DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS},
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowDisplayAffinity, GetWindowThreadProcessId, IsIconic,
        IsWindow, IsWindowVisible,
    },
};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

struct Shot {
    sender: mpsc::SyncSender<Result<DynamicImage>>,
}
impl GraphicsCaptureApiHandler for Shot {
    type Flags = mpsc::SyncSender<Result<DynamicImage>>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    fn new(ctx: Context<Self::Flags>) -> std::result::Result<Self, Self::Error> {
        Ok(Self { sender: ctx.flags })
    }
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> std::result::Result<(), Self::Error> {
        let (w, h) = (frame.width(), frame.height());
        let result = (|| {
            let buffer = frame.buffer().map_err(err)?;
            let mut packed = Vec::new();
            let bytes = buffer.as_nopadding_buffer(&mut packed).to_vec();
            let mut image = RgbaImage::from_raw(w, h, bytes)
                .ok_or("Capture returned an invalid pixel buffer.")?;
            // WGC excludes protected pixels; transparent-only output is unavailable, not valid black content.
            if image.pixels().all(|p| p[3] == 0) {
                return Err("This content cannot be captured (protected or unavailable).".into());
            }
            for p in image.pixels_mut() {
                p[3] = 255;
            }
            Ok(DynamicImage::ImageRgba8(image))
        })();
        let _ = self.sender.try_send(result);
        control.stop();
        Ok(())
    }
    fn on_closed(&mut self) -> std::result::Result<(), Self::Error> {
        let _ = self
            .sender
            .try_send(Err("The capture target closed.".into()));
        Ok(())
    }
}
fn settings<T: TryInto<windows_capture::settings::GraphicsCaptureItemType>>(
    item: T,
    tx: mpsc::SyncSender<Result<DynamicImage>>,
) -> Settings<mpsc::SyncSender<Result<DynamicImage>>, T> {
    Settings::new(
        item,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::Default,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        tx,
    )
}
fn snapshot_monitor(monitor: Monitor) -> Result<DynamicImage> {
    let (tx, rx) = mpsc::sync_channel(1);
    let control = Shot::start_free_threaded(settings(monitor, tx)).map_err(err)?;
    let result = rx
        .recv_timeout(Duration::from_secs(8))
        .map_err(|_| "Display capture timed out or is unavailable.".to_string())
        .and_then(|v| v);
    control.stop().map_err(err)?;
    result
}
fn snapshot_window(id: isize) -> Result<DynamicImage> {
    let hwnd = HWND(id as *mut _);
    eligible(hwnd)?;
    let mut affinity = 0u32;
    // This query only succeeds for layered windows; WGC owns capture access for other targets.
    let _ = unsafe { GetWindowDisplayAffinity(hwnd, &mut affinity) };
    if affinity != 0 {
        return Err("This window is protected from capture.".into());
    }
    snapshot_hwnd(hwnd)
}
fn snapshot_hwnd(hwnd: HWND) -> Result<DynamicImage> {
    let (tx, rx) = mpsc::sync_channel(1);
    let control =
        Shot::start_free_threaded(settings(Window::from_raw_hwnd(hwnd.0), tx)).map_err(err)?;
    let result = rx
        .recv_timeout(Duration::from_secs(8))
        .map_err(|_| {
            "Window capture timed out; this target may be unsupported or protected.".to_string()
        })
        .and_then(|v| v);
    control.stop().map_err(err)?;
    result
}
#[cfg(debug_assertions)]
pub fn verification_screenshot(
    window: &tauri::WebviewWindow,
    path: &std::path::Path,
) -> Result<()> {
    snapshot_hwnd(HWND(window.hwnd().map_err(err)?.0))?
        .save(path)
        .map_err(err)
}
fn eligible(hwnd: HWND) -> Result<RECT> {
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool()
            || !IsWindowVisible(hwnd).as_bool()
            || IsIconic(hwnd).as_bool()
        {
            return Err("Window is unavailable.".into());
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return Err("Captrain cannot capture its own windows.".into());
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4)
            .map_err(err)?;
        if cloaked != 0 {
            return Err("Window is hidden.".into());
        }
        let mut rect = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .map_err(err)?;
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return Err("Window has no capturable area.".into());
        }
        Ok(rect)
    }
}
#[derive(Clone, Serialize)]
pub struct Candidate {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
#[derive(Clone, Serialize)]
pub struct Overlay {
    pub kind: Kind,
    pub image: Option<String>,
    pub width: u32,
    pub height: u32,
    pub windows: Vec<Candidate>,
}
pub struct Pending {
    pub data: Overlay,
    candidates: Vec<isize>,
    sender: Option<mpsc::SyncSender<Option<Selection>>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub window_id: Option<String>,
}
fn acquire(flag: &AtomicBool) -> Result<()> {
    flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| ())
        .map_err(|_| "A capture is already in progress.".to_string())
}

pub fn start(app: &AppHandle, kind: Kind) -> Result<()> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    // The foreground target and physical display are resolved before any app activation.
    let foreground = unsafe { GetForegroundWindow() }.0 as isize;
    let display = shelf_window::pointer_display()?;
    acquire(&state.busy)?;
    state.cancelled.store(false, Ordering::Release);
    state.shelf.lock().unwrap().begin();
    state.timer.notify_one();
    let app = app.clone();
    std::thread::spawn(move || {
        let result = prepare(&app, &state, kind, display, foreground);
        restore_foreground(&app, foreground);
        match result {
            Ok(Some(image)) => commit(&app, &state, kind, image),
            Ok(None) => finish(&app, &state, false, None),
            Err(e) => finish(&app, &state, false, Some(e)),
        }
    });
    Ok(())
}
fn hide_and_flush(app: &AppHandle) -> Result<()> {
    let (tx, rx) = mpsc::sync_channel(1);
    let app2 = app.clone();
    app.run_on_main_thread(move || {
        let result: Result<()> = (|| {
            if let Some(w) = app2.get_webview_window("shelf") {
                w.hide().map_err(err)?;
            }
            if let Some(w) = app2.get_webview_window("overlay") {
                w.hide().map_err(err)?;
            }
            Ok(())
        })();
        let _ = tx.send(result);
    })
    .map_err(err)?;
    rx.recv_timeout(Duration::from_secs(5)).map_err(err)??;
    // DwmFlush waits for the next present of submitted compositor updates, not an arbitrary sleep.
    unsafe {
        DwmFlush().map_err(err)?;
    }
    Ok(())
}
fn restore_foreground(app: &AppHandle, foreground: isize) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Ok(hwnd) = overlay.hwnd() {
            unsafe {
                if GetForegroundWindow().0 == hwnd.0
                    && IsWindowVisible(HWND(foreground as *mut _)).as_bool()
                {
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(HWND(
                        foreground as *mut _,
                    ));
                }
            }
        }
    }
}
fn prepare(
    app: &AppHandle,
    state: &Arc<AppState>,
    kind: Kind,
    d: DisplayInfo,
    foreground: isize,
) -> Result<Option<DynamicImage>> {
    shelf_window::apply(app, false);
    hide_and_flush(app)?;
    let image = snapshot_monitor(d.monitor)?;
    if image.width() != (d.bounds.right - d.bounds.left) as u32
        || image.height() != (d.bounds.bottom - d.bounds.top) as u32
    {
        return Err("Display geometry changed during capture. Please try again.".into());
    }
    if state.cancelled.load(Ordering::Acquire) {
        return Ok(None);
    }
    if kind == Kind::Screen {
        return Ok(Some(image));
    }
    let mut candidates = vec![];
    let mut windows = vec![];
    if kind == Kind::Window {
        for window in Window::enumerate().map_err(err)? {
            let hwnd = HWND(window.as_raw_hwnd());
            if let Ok(r) = eligible(hwnd) {
                if r.right <= d.bounds.left
                    || r.left >= d.bounds.right
                    || r.bottom <= d.bounds.top
                    || r.top >= d.bounds.bottom
                {
                    continue;
                }
                candidates.push(hwnd.0 as isize);
                windows.push(Candidate {
                    id: (hwnd.0 as isize).to_string(),
                    x: r.left - d.bounds.left,
                    y: r.top - d.bounds.top,
                    width: r.right - r.left,
                    height: r.bottom - r.top,
                });
            }
        }
        if windows.is_empty() {
            return Err("There are no available windows on this display.".into());
        }
    }
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(err)?;
    let data = Overlay {
        kind,
        image: Some(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(png.into_inner())
        )),
        width: image.width(),
        height: image.height(),
        windows,
    };
    let (selection_tx, selection_rx) = mpsc::sync_channel(1);
    *state.pending.lock().unwrap() = Some(Pending {
        data,
        candidates,
        sender: Some(selection_tx),
    });
    let app2 = app.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result: Result<()> = (|| {
            let window = WebviewWindowBuilder::new(
                &app2,
                "overlay",
                WebviewUrl::App("index.html?overlay=1".into()),
            )
            .title("Captrain selection")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build()
            .map_err(err)?;
            window
                .set_position(PhysicalPosition::new(d.bounds.left, d.bounds.top))
                .map_err(err)?;
            window
                .set_size(PhysicalSize::new(
                    (d.bounds.right - d.bounds.left) as u32,
                    (d.bounds.bottom - d.bounds.top) as u32,
                ))
                .map_err(err)?;
            window.show().map_err(err)?;
            window.set_focus().map_err(err)?;
            Ok(())
        })();
        let _ = tx.send(result);
    })
    .map_err(err)?;
    rx.recv_timeout(Duration::from_secs(10)).map_err(err)??;
    if state.cancelled.load(Ordering::Acquire) {
        return Ok(None);
    }
    let selection = selection_rx.recv().map_err(err)?;
    state.pending.lock().unwrap().take();
    let Some(selection) = selection else {
        return Ok(None);
    };
    if state.cancelled.load(Ordering::Acquire) {
        return Ok(None);
    }
    restore_foreground(app, foreground);
    hide_and_flush(app)?;
    if kind == Kind::Region {
        Ok(Some(crop(&image, &selection)?))
    } else {
        drop(image);
        let id = selection
            .window_id
            .as_deref()
            .ok_or("Choose a window.")?
            .parse()
            .map_err(err)?;
        snapshot_window(id).map(Some)
    }
}
fn validate_rect(width: u32, height: u32, selection: &Selection) -> Result<(u32, u32, u32, u32)> {
    let [Some(x), Some(y), Some(w), Some(h)] =
        [selection.x, selection.y, selection.width, selection.height]
    else {
        return Err("Select a rectangular region.".into());
    };
    if [x, y, w, h].iter().any(|v| !v.is_finite()) || x < 0.0 || y < 0.0 || w < 1.0 || h < 1.0 {
        return Err("Select a region at least one pixel wide and high.".into());
    }
    let (x, y, w, h) = (
        x.round() as u32,
        y.round() as u32,
        w.round() as u32,
        h.round() as u32,
    );
    if x.checked_add(w).is_none_or(|v| v > width) || y.checked_add(h).is_none_or(|v| v > height) {
        return Err("Selection is outside the display.".into());
    }
    Ok((x, y, w, h))
}
pub fn crop(image: &DynamicImage, selection: &Selection) -> Result<DynamicImage> {
    let (x, y, w, h) = validate_rect(image.width(), image.height(), selection)?;
    Ok(image.crop_imm(x, y, w, h))
}
pub fn select(app: &AppHandle, selection: Selection) -> Result<()> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    let mut lock = state.pending.lock().unwrap();
    let pending = lock.as_mut().ok_or("No capture selection is active.")?;
    // Validate before consuming the selection so a bad rectangle can be corrected.
    let kind = pending.data.kind;
    if kind == Kind::Region {
        validate_rect(pending.data.width, pending.data.height, &selection)?;
    }
    if kind == Kind::Window {
        let id: isize = selection
            .window_id
            .as_deref()
            .ok_or("Choose a window.")?
            .parse()
            .map_err(err)?;
        if !pending.candidates.contains(&id) {
            return Err("That window is not a capture candidate.".into());
        }
    }
    let sender = pending
        .sender
        .take()
        .ok_or("Selection has already been submitted.")?;
    sender.send(Some(selection)).map_err(err)
}
fn commit(app: &AppHandle, state: &Arc<AppState>, kind: Kind, image: DynamicImage) {
    if state.cancelled.load(Ordering::Acquire) {
        finish(app, state, false, None);
        return;
    }
    let started = Instant::now();
    let result = state.library.lock().unwrap().commit(kind, image);
    match result {
        Ok(item) => {
            eprintln!(
                "capture commit+thumbnail: {}ms",
                started.elapsed().as_millis()
            );
            let _ = app.emit("capture-created", &item);
            finish(app, state, true, None);
        }
        Err(e) => finish(
            app,
            state,
            false,
            Some(format!("Could not save capture: {e}")),
        ),
    }
}
pub fn finish(app: &AppHandle, state: &Arc<AppState>, committed: bool, error: Option<String>) {
    state.pending.lock().unwrap().take();
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.destroy();
    }
    state
        .shelf
        .lock()
        .unwrap()
        .finish(committed, error, Instant::now());
    state.timer.notify_one();
    state.busy.store(false, Ordering::Release);
    shelf_window::apply(app, false);
}
pub fn cancel(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>();
    if !state.busy.load(Ordering::Acquire) {
        return;
    }
    state.cancelled.store(true, Ordering::Release);
    // Only the pending selector can finish immediately; acquisition/commit retains the transaction guard.
    if let Some(pending) = state.pending.lock().unwrap().as_mut() {
        if let Some(sender) = pending.sender.take() {
            let _ = sender.send(None);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transaction_guard_rejects_overlap() {
        let flag = AtomicBool::new(false);
        acquire(&flag).unwrap();
        assert!(acquire(&flag).is_err());
        flag.store(false, Ordering::Release);
        assert!(acquire(&flag).is_ok());
    }
    #[test]
    fn physical_crop_bounds() {
        let image = DynamicImage::new_rgba8(150, 100);
        let mut s = Selection {
            x: Some(30.0),
            y: Some(20.0),
            width: Some(60.0),
            height: Some(40.0),
            window_id: None,
        };
        let crop = crop(&image, &s).unwrap();
        assert_eq!((crop.width(), crop.height()), (60, 40));
        s.width = Some(0.0);
        assert!(super::crop(&image, &s).is_err());
        s.width = Some(200.0);
        assert!(super::crop(&image, &s).is_err());
        s.width = Some(f64::NAN);
        assert!(super::crop(&image, &s).is_err());
    }
}
