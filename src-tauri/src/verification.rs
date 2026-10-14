//! Opt-in native development check, isolated from the real library. No release test controls.
use crate::{
    capture,
    library::{err, Kind, Result},
    shelf_window::{self, Mode},
    AppState,
};
use std::{
    fs,
    path::Path,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

fn wait(label: &str, condition: impl Fn() -> bool) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(20);
    while !condition() {
        if Instant::now() > until {
            return Err(format!("Timed out: {label}"));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}
pub fn start(app: &AppHandle) {
    let Some(output) = std::env::var_os("CAPTRAIN_VERIFY") else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let output = std::path::PathBuf::from(output);
        let result = run(&app, &output);
        let report = match result {
            Ok(report) => report,
            Err(e) => format!("FAILED: {e}\n"),
        };
        let _ = fs::create_dir_all(&output);
        let _ = fs::write(output.join("native-results.txt"), &report);
        eprintln!("NATIVE VERIFICATION: {report}");
        app.exit(if report.starts_with("FAILED") { 1 } else { 0 });
    });
}
fn run(app: &AppHandle, output: &Path) -> Result<String> {
    fs::create_dir_all(output).map_err(err)?;
    let state = app.state::<Arc<AppState>>();
    wait("frontend ready", || state.shelf.lock().unwrap().ready)?;
    let window = app.get_webview_window("shelf").ok_or("Missing shelf")?;
    let display = shelf_window::pointer_display()?;
    let mut results = vec![format!(
        "Native debug checks; display {}x{}, scale {}",
        display.bounds.right - display.bounds.left,
        display.bounds.bottom - display.bounds.top,
        display.scale
    )];
    wait("collapsed size", || {
        window
            .inner_size()
            .is_ok_and(|s| s.width <= (16.0 * display.scale) as u32 + 1)
    })?;
    results
        .push("PASS: frontend IPC ready; launch collapsed with tight 16x72 logical window".into());
    let foreground = unsafe { GetForegroundWindow() }.0 as isize;
    capture::verification_screenshot(&window, &output.join("bar.png"))?;
    capture::start(app, Kind::Screen)?;
    wait("screen capture commit", || {
        !state.busy.load(Ordering::Acquire)
    })?;
    let page = state.library.lock().unwrap().list("", 0, 20);
    if page.total != 1 {
        return Err(format!(
            "Screen capture did not commit: {:?}",
            state.shelf.lock().unwrap().view.error
        ));
    }
    if page.items[0].width != (display.bounds.right - display.bounds.left) as u32
        || page.items[0].height != (display.bounds.bottom - display.bounds.top) as u32
    {
        return Err("Screen dimensions mismatch".into());
    }
    if unsafe { GetForegroundWindow() }.0 as isize != foreground {
        return Err("Temporary screen reveal changed foreground window".into());
    }
    results.push("PASS: real WGC screen capture, native dimensions, committed thumbnail/history; temporary reveal preserves foreground".into());
    shelf_window::action(app, "hold")?;
    wait("shelf rendered", || {
        window.inner_size().is_ok_and(|s| s.width > 100)
    })?;
    std::thread::sleep(Duration::from_millis(500));
    capture::verification_screenshot(&window, &output.join("shelf.png"))?;
    shelf_window::action(app, "release")?;
    shelf_window::action(app, "leave")?;
    wait("three-second dismissal", || {
        state.shelf.lock().unwrap().view.mode == Mode::Collapsed
    })?;
    results
        .push("PASS: temporary reveal expires after ready; native shelf screenshot saved".into());
    capture::start(app, Kind::Region)?;
    wait("region selector", || {
        state.pending.lock().unwrap().is_some()
    })?;
    capture::cancel(app);
    wait("cancelled transaction", || {
        !state.busy.load(Ordering::Acquire)
    })?;
    if state.library.lock().unwrap().list("", 0, 20).total != 1 {
        return Err("Cancellation changed history".into());
    }
    if state.shelf.lock().unwrap().view.mode != Mode::Collapsed {
        return Err("Cancellation failed to restore collapsed mode".into());
    }
    results.push(
        "PASS: region cancellation restores collapsed state and leaves history unchanged".into(),
    );
    capture::start(app, Kind::Region)?;
    if capture::start(app, Kind::Region).is_ok() {
        return Err("Overlapping capture accepted".into());
    }
    wait("region selector", || {
        state.pending.lock().unwrap().is_some()
    })?;
    capture::select(
        app,
        capture::Selection {
            x: Some(30.0),
            y: Some(40.0),
            width: Some(240.0),
            height: Some(160.0),
            window_id: None,
        },
    )?;
    wait("region commit", || !state.busy.load(Ordering::Acquire))?;
    let page = state.library.lock().unwrap().list("region", 0, 20);
    if page.total != 1 || page.items[0].width != 240 || page.items[0].height != 160 {
        return Err(format!(
            "Region did not commit: {:?}",
            state.shelf.lock().unwrap().view.error
        ));
    }
    results.push(
        "PASS: region physical crop 240x160, native overlap guard, commit and real thumbnail"
            .into(),
    );
    shelf_window::action(app, "open")?;
    std::thread::sleep(Duration::from_millis(250));
    window
        .eval("document.querySelector('button[aria-label=Settings]')?.click()")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(250));
    capture::verification_screenshot(&window, &output.join("settings.png"))?;
    let root = state.library.lock().unwrap().root.clone();
    if crate::library::Library::open(root.clone())?
        .list("", 0, 20)
        .total
        != 2
    {
        return Err("Reopen lost committed history".into());
    }
    results.push("PASS: on-disk library reopens with both committed captures".into());
    let previous = state.settings.lock().unwrap().value.clone();
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL};
        RegisterHotKey(None, 0x7311, MOD_ALT | MOD_CONTROL, 0x7B).map_err(err)?;
    }
    let mut conflicting = previous.clone();
    conflicting
        .shortcuts
        .insert("region".into(), "Ctrl+Alt+F12".into());
    let conflict = crate::shortcuts::update(app, conflicting);
    unsafe {
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey(None, 0x7311);
    }
    if conflict.is_ok() || state.settings.lock().unwrap().value.shortcuts != previous.shortcuts {
        return Err("Native shortcut conflict failed to preserve working bindings".into());
    }
    results.push(
        "PASS: actual OS shortcut conflict rejects edit and preserves previous bindings".into(),
    );
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL};
        RegisterHotKey(None, 0x7311, MOD_ALT | MOD_CONTROL, 0x7B).map_err(err)?;
    }
    window
        .eval("document.querySelector('.keycap')?.click()")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(250));
    app.emit_to("shelf", "shortcut-recorded", "Ctrl+Alt+F12")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(250));
    capture::verification_screenshot(&window, &output.join("settings-conflict.png"))?;
    unsafe {
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey(None, 0x7311);
    }
    window
        .eval("document.querySelector('button[aria-label=Back]')?.click()")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(250));
    window
        .eval("document.querySelector('.row')?.click()")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(250));
    capture::verification_screenshot(&window, &output.join("preview.png"))?;
    window
        .eval("document.querySelector('.btn.danger')?.click()")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(250));
    capture::verification_screenshot(&window, &output.join("delete-confirmation.png"))?;
    shelf_window::action(app, "collapse")?;
    capture::start(app, Kind::Window)?;
    wait("window selector", || {
        state.pending.lock().unwrap().is_some() || !state.busy.load(Ordering::Acquire)
    })?;
    let target = state
        .pending
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|p| p.data.windows.first().map(|w| w.id.clone()))
        .ok_or_else(|| {
            format!(
                "Window selector has no target: {:?}",
                state.shelf.lock().unwrap().view.error
            )
        })?;
    capture::select(
        app,
        capture::Selection {
            x: None,
            y: None,
            width: None,
            height: None,
            window_id: Some(target),
        },
    )?;
    wait("native window commit", || {
        !state.busy.load(Ordering::Acquire)
    })?;
    if state.library.lock().unwrap().list("window", 0, 20).total != 1 {
        return Err(format!(
            "Window capture failed: {:?}",
            state.shelf.lock().unwrap().view.error
        ));
    }
    results
        .push("PASS: eligible external window picker and WGC native window capture commit".into());
    // Keep the fixture outside the real profile and retain only the latest page in the renderer.
    shelf_window::action(app, "collapse")?;
    fs::write(output.join("idle-ready"), "ready").map_err(err)?;
    std::thread::sleep(Duration::from_secs(65));
    fs::write(output.join("cycles-start"), "start").map_err(err)?;
    let cycles = Instant::now();
    for _ in 0..100 {
        capture::start(app, Kind::Region)?;
        wait("cycle region selector", || {
            state.pending.lock().unwrap().is_some()
        })?;
        capture::select(
            app,
            capture::Selection {
                x: Some(30.0),
                y: Some(40.0),
                width: Some(240.0),
                height: Some(160.0),
                window_id: None,
            },
        )?;
        wait("cycle commit", || !state.busy.load(Ordering::Acquire))?;
        let item = state.library.lock().unwrap().list("", 0, 1).items[0].clone();
        let (source, _) = state.library.lock().unwrap().export_source(&item.id)?;
        drop(image::open(source).map_err(err)?);
        shelf_window::action(app, "collapse")?;
    }
    results.push(format!(
        "PASS: 100 real region acquisition/commit/decode/collapse cycles in {}ms",
        cycles.elapsed().as_millis()
    ));
    fs::write(output.join("cycles-end"), "end").map_err(err)?;
    std::thread::sleep(Duration::from_secs(10));
    results.push("NOT VERIFIED: real pointer/keyboard input, clipboard, Save dialog, mixed-DPI/HDR hardware, protected content, release performance".into());
    results.push(format!("Isolated test captures: {}", root.display()));
    Ok(results.join("\n"))
}
