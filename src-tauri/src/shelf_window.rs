use crate::{
    library::{err, Result},
    AppState,
};
use serde::Serialize;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    },
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{
            GetCursorPos, SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SW_SHOWNOACTIVATE,
        },
    },
};
use windows_capture::monitor::Monitor;

#[derive(Clone, Copy, Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Collapsed,
    Temporary,
    Manual,
    Capturing,
}
#[derive(Clone, Serialize)]
pub struct View {
    pub mode: Mode,
    pub hovered: bool,
    pub error: Option<String>,
    pub reveal: u64,
}
pub struct Shelf {
    pub view: View,
    pub deadline: Option<Instant>,
    previous: Mode,
    holds: u32,
    pub ready: bool,
    pub dragging: bool,
    waiting_visible: bool,
}
impl Shelf {
    pub fn new(error: Option<String>) -> Self {
        Self {
            view: View {
                mode: Mode::Collapsed,
                hovered: false,
                error,
                reveal: 0,
            },
            deadline: None,
            previous: Mode::Collapsed,
            holds: 0,
            ready: false,
            dragging: false,
            waiting_visible: false,
        }
    }
    fn arm(&mut self, now: Instant) {
        self.deadline = if self.view.mode == Mode::Temporary
            && !self.view.hovered
            && self.holds == 0
            && !self.waiting_visible
        {
            Some(now + Duration::from_secs(3))
        } else {
            None
        };
    }
    pub fn action(&mut self, action: &str, now: Instant) -> Result<()> {
        match action {
            "ready" => {
                if !self.ready || self.waiting_visible {
                    self.ready = true;
                    self.waiting_visible = false;
                    self.arm(now);
                }
            }
            "enter" => {
                self.view.hovered = true;
                self.arm(now);
            }
            "leave" => {
                self.view.hovered = false;
                self.arm(now);
            }
            "hold" => {
                self.holds += 1;
                self.arm(now);
            }
            "release" => {
                self.holds = self.holds.saturating_sub(1);
                self.arm(now);
            }
            "clearError" => self.view.error = None,
            "open" | "interact" if self.view.mode != Mode::Capturing => {
                self.view.mode = Mode::Manual;
                self.deadline = None;
            }
            "collapse" if self.view.mode != Mode::Capturing => self.collapse(),
            "toggle" if self.view.mode != Mode::Capturing => {
                if self.view.mode == Mode::Manual {
                    self.collapse();
                } else {
                    self.view.mode = Mode::Manual;
                    self.deadline = None;
                }
            }
            "open" | "interact" | "collapse" | "toggle" => {}
            _ => return Err("Unknown shelf action.".into()),
        }
        Ok(())
    }
    pub fn action_for(&mut self, action: &str, reveal: Option<u64>, now: Instant) -> Result<bool> {
        if action == "ready" && reveal.is_some_and(|r| r != self.view.reveal) {
            return Ok(false);
        }
        self.action(action, now)?;
        Ok(true)
    }
    fn collapse(&mut self) {
        self.view.mode = Mode::Collapsed;
        self.view.hovered = false;
        self.holds = 0;
        self.deadline = None;
        self.waiting_visible = false;
    }
    pub fn begin(&mut self) {
        self.previous = self.view.mode;
        self.view.mode = Mode::Capturing;
        self.deadline = None;
        self.view.hovered = false;
    }
    pub fn finish(
        &mut self,
        committed: bool,
        error: Option<String>,
        now: Instant,
        shortcuts_only: bool,
    ) {
        self.view.mode = if shortcuts_only && (committed || error.is_some()) {
            Mode::Temporary
        } else if self.previous == Mode::Manual {
            Mode::Manual
        } else if committed || error.is_some() {
            Mode::Temporary
        } else {
            self.previous
        };
        self.view.error = error.or_else(|| self.view.error.take());
        self.waiting_visible = self.view.mode == Mode::Temporary;
        if self.waiting_visible {
            self.view.reveal += 1;
        }
        self.arm(now);
    }
    pub fn expire(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|d| now >= d) && self.view.mode == Mode::Temporary {
            self.collapse();
            true
        } else {
            false
        }
    }
}

pub struct DisplayInfo {
    pub monitor: Monitor,
    pub work: windows::Win32::Foundation::RECT,
    pub bounds: windows::Win32::Foundation::RECT,
    pub scale: f64,
    pub id: String,
}
pub fn displays() -> Result<Vec<DisplayInfo>> {
    Monitor::enumerate()
        .map_err(err)?
        .into_iter()
        .map(|monitor| {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let hmon = HMONITOR(monitor.as_raw_hmonitor());
            if !unsafe { GetMonitorInfoW(hmon, &mut info) }.as_bool() {
                return Err("Display is unavailable.".into());
            }
            let (mut x, mut y) = (96, 96);
            unsafe {
                GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut x, &mut y).map_err(err)?;
            }
            let id = monitor.device_name().map_err(err)?;
            Ok(DisplayInfo {
                monitor,
                work: info.rcWork,
                bounds: info.rcMonitor,
                scale: x as f64 / 96.0,
                id,
            })
        })
        .collect()
}
pub fn pointer_display() -> Result<DisplayInfo> {
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point).map_err(err)?;
    }
    let target = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTOPRIMARY) };
    displays()?
        .into_iter()
        .find(|d| d.monitor.as_raw_hmonitor() == target.0)
        .ok_or("No display is available.".into())
}
fn edge_y(top: i32, bottom: i32, height: i32, position: f64) -> i32 {
    top + ((bottom - top - height).max(0) as f64 * position.clamp(0.0, 1.0)).round() as i32
}

fn drag_position(
    work: &windows::Win32::Foundation::RECT,
    scale: f64,
    point: POINT,
    grab: f64,
) -> (&'static str, f64) {
    let edge = if point.x - work.left < work.right - point.x {
        "left"
    } else {
        "right"
    };
    let height = (72.0 * scale) as i32;
    let travel = (work.bottom - work.top - height).max(0);
    let position = if travel == 0 {
        0.5
    } else {
        ((point.y - work.top) as f64 - height as f64 * grab) / travel as f64
    };
    (edge, position.clamp(0.0, 1.0))
}

#[tauri::command]
pub async fn drag_shelf(window: tauri::WebviewWindow, app: AppHandle) -> Result<bool> {
    crate::shelf_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
        let state = app.state::<Arc<AppState>>();
        state
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Finish the current operation before moving the bar.".to_string())?;
        let result = (|| {
            {
                let mut shelf = state.shelf.lock().unwrap();
                if shelf.view.mode != Mode::Collapsed {
                    return Ok(false);
                }
                shelf.dragging = true;
            }
            let all = displays()?;
            let mut start = POINT::default();
            unsafe {
                GetCursorPos(&mut start).map_err(err)?;
            }
            let origin = window.outer_position().map_err(err)?;
            let size = window.outer_size().map_err(err)?;
            let grab = ((start.y - origin.y) as f64 / size.height.max(1) as f64).clamp(0.0, 1.0);
            let threshold = 5.0 * window.scale_factor().map_err(err)?;
            let mut target = None;
            while unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } < 0 {
                let mut point = POINT::default();
                unsafe {
                    GetCursorPos(&mut point).map_err(err)?;
                }
                if target.is_some()
                    || ((point.x - start.x) as f64).hypot((point.y - start.y) as f64) >= threshold
                {
                    let display = all
                        .iter()
                        .min_by_key(|d| {
                            let x = (point.x as i64
                                - point.x.clamp(d.bounds.left, d.bounds.right) as i64)
                                .abs();
                            let y = (point.y as i64
                                - point.y.clamp(d.bounds.top, d.bounds.bottom) as i64)
                                .abs();
                            x * x + y * y
                        })
                        .ok_or("No display is available.")?;
                    let (edge, position) = drag_position(&display.work, display.scale, point, grab);
                    let width = (16.0 * display.scale) as i32;
                    let height = (72.0 * display.scale) as i32;
                    let x = if edge == "left" {
                        display.work.left
                    } else {
                        display.work.right - width
                    };
                    window
                        .set_size(PhysicalSize::new(width as u32, height as u32))
                        .map_err(err)?;
                    window
                        .set_position(PhysicalPosition::new(
                            x,
                            edge_y(display.work.top, display.work.bottom, height, position),
                        ))
                        .map_err(err)?;
                    target = Some((display.id.clone(), edge.to_string(), position));
                }
                std::thread::sleep(Duration::from_millis(16));
            }
            if let Some((display, edge, position)) = target {
                let mut next = state.settings.lock().unwrap().value.clone();
                next.display = Some(display);
                next.edge = edge;
                next.edge_position = position;
                crate::shortcuts::update(&app, next)?;
                Ok(true)
            } else {
                Ok(false)
            }
        })();
        state.shelf.lock().unwrap().dragging = false;
        state.busy.store(false, Ordering::Release);
        apply(&app, false);
        result
    })
    .await
    .map_err(err)?
}

pub fn apply(app: &AppHandle, activate: bool) {
    let state = app.state::<Arc<AppState>>();
    let shelf = state.shelf.lock().unwrap();
    let view = shelf.view.clone();
    let ready = shelf.ready;
    drop(shelf);
    let _ = app.emit("shelf-state", &view);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(window) = app2.get_webview_window("shelf") else {
            return;
        };
        if view.mode == Mode::Capturing || !ready {
            let _ = window.hide();
            return;
        }
        let state = app2.state::<Arc<AppState>>();
        if state.shelf.lock().unwrap().dragging {
            return;
        }
        let settings = state.settings.lock().unwrap().value.clone();
        if settings.shortcuts_only && view.mode == Mode::Collapsed {
            let _ = window.hide();
            return;
        }
        let Ok(mut all) = displays() else {
            return;
        };
        let chosen = settings
            .display
            .as_ref()
            .and_then(|id| all.iter().position(|d| &d.id == id))
            .unwrap_or_else(|| {
                all.iter()
                    .position(|d| d.bounds.left == 0 && d.bounds.top == 0)
                    .unwrap_or(0)
            });
        if all.is_empty() {
            return;
        }
        let d = all.swap_remove(chosen);
        let collapsed = view.mode == Mode::Collapsed;
        let (width, height) = if collapsed {
            (16.0, 72.0)
        } else if settings.shortcuts_only && view.mode == Mode::Temporary {
            (340.0, 120.0)
        } else {
            (540.0, 740.0)
        };
        let width = ((width * d.scale) as i32).min(d.work.right - d.work.left);
        let height = ((height * d.scale) as i32).min(d.work.bottom - d.work.top);
        let inset = if collapsed {
            0
        } else {
            (10.0 * d.scale) as i32
        };
        let x = if settings.edge == "left" {
            d.work.left + inset
        } else {
            d.work.right - width - inset
        }
        .clamp(d.work.left, d.work.right - width);
        let y = edge_y(d.work.top, d.work.bottom, height, settings.edge_position);
        let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));
        let _ = window.set_position(PhysicalPosition::new(x, y));
        if let Ok(handle) = window.hwnd() {
            unsafe {
                let hwnd = HWND(handle.0);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                );
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
        }
        if activate && view.mode == Mode::Manual {
            let _ = window.set_focus();
        }
    });
}
pub fn action(app: &AppHandle, action: &str) -> Result<()> {
    action_for(app, action, None)
}
pub fn action_for(app: &AppHandle, action: &str, reveal: Option<u64>) -> Result<()> {
    let state = app.state::<Arc<AppState>>();
    let mut shelf = state.shelf.lock().unwrap();
    let initial_ready = !shelf.ready;
    if !shelf.action_for(action, reveal, Instant::now())? {
        return Ok(());
    }
    drop(shelf);
    state.timer.notify_one();
    if action == "ready" && !initial_ready {
        return Ok(());
    }
    apply(app, matches!(action, "open" | "toggle" | "interact"));
    Ok(())
}
pub fn timer(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let mut shelf = state.shelf.lock().unwrap();
        loop {
            if shelf.expire(Instant::now()) {
                drop(shelf);
                apply(&app, false);
                shelf = state.shelf.lock().unwrap();
            }
            shelf = if let Some(d) = shelf.deadline {
                state
                    .timer
                    .wait_timeout(shelf, d.saturating_duration_since(Instant::now()))
                    .unwrap()
                    .0
            } else {
                state.timer.wait(shelf).unwrap()
            };
        }
    });
}

// Windows sends this when displays disconnect/reconfigure, even while the shelf is collapsed.
pub fn watch_displays(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("shelf")
        .ok_or("Shelf window is missing.")?;
    let handle = window.hwnd().map_err(err)?;
    let reference = Box::into_raw(Box::new(app.clone()));
    if !unsafe {
        windows::Win32::UI::Shell::SetWindowSubclass(
            HWND(handle.0),
            Some(display_proc),
            1,
            reference as usize,
        )
    }
    .as_bool()
    {
        unsafe {
            drop(Box::from_raw(reference));
        }
        return Err("Could not listen for display changes.".into());
    }
    Ok(())
}
unsafe extern "system" fn display_proc(
    hwnd: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    id: usize,
    data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass},
        WindowsAndMessaging::{WM_DISPLAYCHANGE, WM_NCDESTROY},
    };
    if message == WM_DISPLAYCHANGE {
        let app = &*(data as *const AppHandle);
        apply(app, false);
    }
    if message == WM_NCDESTROY {
        let _ = RemoveWindowSubclass(hwnd, Some(display_proc), id);
        drop(Box::from_raw(data as *mut AppHandle));
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_placement_clamps_to_work_area_and_handles_negative_monitors() {
        use windows::Win32::Foundation::RECT;
        let work = RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        assert_eq!(edge_y(-200, 880, 72, 0.0), -200);
        assert_eq!(edge_y(-200, 880, 72, 1.0), 808);
        assert_eq!(edge_y(-200, 880, 740, 0.5), -30);
        assert_eq!(edge_y(0, 50, 72, 1.0), 0);
        assert_eq!(
            drag_position(&work, 1.0, POINT { x: -1910, y: -500 }, 0.5),
            ("left", 0.0)
        );
        assert_eq!(
            drag_position(&work, 1.5, POINT { x: -10, y: 1000 }, 0.5),
            ("right", 1.0)
        );
        assert_eq!(
            drag_position(&work, 1.5, POINT { x: -10, y: 340 }, 0.5),
            ("right", 0.5)
        );
    }
    #[test]
    fn shortcuts_only_capture_uses_temporary_toast_even_after_manual_open() {
        let now = Instant::now();
        let mut shelf = Shelf::new(None);
        shelf.action("open", now).unwrap();
        shelf.begin();
        shelf.finish(true, None, now, true);
        assert_eq!(shelf.view.mode, Mode::Temporary);
        shelf.action("ready", now).unwrap();
        assert!(shelf.expire(now + Duration::from_secs(3)));
        assert_eq!(shelf.view.mode, Mode::Collapsed);
        shelf.action("open", now).unwrap();
        shelf.begin();
        shelf.finish(false, None, now, true);
        assert_eq!(shelf.view.mode, Mode::Manual);
        shelf.begin();
        shelf.finish(false, Some("Capture failed".into()), now, true);
        assert_eq!(shelf.view.mode, Mode::Temporary);
        assert_eq!(shelf.view.error.as_deref(), Some("Capture failed"));
    }
    #[test]
    fn obsolete_render_ready_cannot_arm_newer_reveal() {
        let now = Instant::now();
        let mut shelf = Shelf::new(None);
        shelf.action("ready", now).unwrap();
        shelf.begin();
        shelf.finish(true, None, now, false);
        let old = shelf.view.reveal;
        shelf.begin();
        shelf.finish(true, None, now, false);
        assert!(!shelf.action_for("ready", Some(old), now).unwrap());
        assert!(shelf.deadline.is_none());
        let current = shelf.view.reveal;
        assert!(shelf.action_for("ready", Some(current), now).unwrap());
        let deadline = shelf.deadline;
        shelf
            .action_for("ready", Some(current), now + Duration::from_secs(2))
            .unwrap();
        assert_eq!(shelf.deadline, deadline);
    }
    #[test]
    fn replacement_hover_manual_and_holds() {
        let t = Instant::now();
        let mut s = Shelf::new(None);
        s.begin();
        s.finish(true, None, t, false);
        assert!(!s.expire(t + Duration::from_secs(10)));
        s.action("ready", t).unwrap();
        assert!(!s.expire(t + Duration::from_millis(2999)));
        s.action("enter", t).unwrap();
        assert!(!s.expire(t + Duration::from_secs(30)));
        s.action("leave", t + Duration::from_secs(30)).unwrap();
        assert!(!s.expire(t + Duration::from_secs(32)));
        s.begin();
        s.finish(true, None, t, false);
        s.action("ready", t + Duration::from_secs(32)).unwrap();
        assert!(!s.expire(t + Duration::from_secs(33)));
        assert!(s.expire(t + Duration::from_secs(35)));
        s.begin();
        s.finish(true, None, t, false);
        s.action("interact", t).unwrap();
        assert!(!s.expire(t + Duration::from_secs(100)));
        s.begin();
        s.finish(true, None, t, false);
        assert_eq!(s.view.mode, Mode::Manual);
        s.action("collapse", t).unwrap();
        s.begin();
        s.finish(true, None, t, false);
        s.action("ready", t).unwrap();
        s.action("hold", t).unwrap();
        assert!(!s.expire(t + Duration::from_secs(10)));
        s.action("release", t).unwrap();
        assert!(s.expire(t + Duration::from_secs(3)));
    }
    #[test]
    fn cancellation_restores_previous_mode() {
        let t = Instant::now();
        for mode in [Mode::Collapsed, Mode::Temporary, Mode::Manual] {
            let mut s = Shelf::new(None);
            s.view.mode = mode;
            s.begin();
            s.finish(false, None, t, false);
            assert_eq!(s.view.mode, mode);
        }
    }
}
