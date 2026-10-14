use crate::library::Result;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::{
    Foundation::HWND,
    UI::{
        Accessibility::{SetWinEventHook, HWINEVENTHOOK},
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, EVENT_SYSTEM_FOREGROUND,
            WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
        },
    },
};

static EXTERNAL: AtomicIsize = AtomicIsize::new(0);
fn remember(hwnd: HWND) {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid != 0 && pid != std::process::id() {
        EXTERNAL.store(hwnd.0 as isize, Ordering::Release);
    }
}
unsafe extern "system" fn changed(
    _: HWINEVENTHOOK,
    _: u32,
    hwnd: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    remember(hwnd);
}
pub fn start() -> Result<()> {
    remember(unsafe { GetForegroundWindow() });
    // Installed on Tauri's message-loop thread; Windows releases this hook when the process exits.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.0.is_null() {
        return Err("Could not track the active window for direct capture.".into());
    }
    Ok(())
}
pub fn target() -> isize {
    remember(unsafe { GetForegroundWindow() });
    EXTERNAL.load(Ordering::Acquire)
}
