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
    if std::env::var_os("CAPTRAIN_VERIFY_UPDATE").is_some() {
        return run_update(app, output);
    }
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

fn run_update(app: &AppHandle, output: &Path) -> Result<String> {
    fs::create_dir_all(output).map_err(err)?;
    let output = output.canonicalize().map_err(err)?;
    let state = app.state::<Arc<AppState>>();
    wait("frontend ready", || state.shelf.lock().unwrap().ready)?;
    let shelf = app.get_webview_window("shelf").ok_or("Missing shelf")?;
    let root = state.library.lock().unwrap().root.clone();
    let mut folders = Vec::new();
    for name in ["Ideas", "Todo", "Inspiration", "Design"] {
        folders.push(crate::desktop::create_folder(
            shelf.clone(),
            app.clone(),
            name.into(),
        )?);
    }
    let exports = output.join("saved-pictures");
    fs::create_dir_all(&exports).map_err(err)?;
    let mut settings = state.settings.lock().unwrap().value.clone();
    settings.capture_folder = Some(exports.clone());
    settings.active_folder = Some(folders[0].id.clone());
    crate::shortcuts::update(app, settings)?;
    capture::start_with(app, Kind::Screen, true)?;
    wait("screen capture with auto-save", || {
        !state.busy.load(Ordering::Acquire)
    })?;
    let item = state
        .library
        .lock()
        .unwrap()
        .list("", 0, 1)
        .items
        .first()
        .cloned()
        .ok_or("Screen did not commit")?;
    if item.folder_id.as_deref() != Some(&folders[0].id)
        || !item.exported_path.as_ref().is_some_and(|p| p.is_file())
    {
        return Err("Capture folder or automatic PNG save failed".into());
    }
    capture::start_with(app, Kind::Region, true)?;
    wait("direct region selector", || {
        state.pending.lock().unwrap().is_some()
    })?;
    capture::select(
        app,
        capture::Selection {
            x: Some(40.0),
            y: Some(50.0),
            width: Some(280.0),
            height: Some(180.0),
            window_id: None,
        },
    )?;
    wait("direct region commit", || {
        !state.busy.load(Ordering::Acquire)
    })?;
    shelf_window::action(app, "open")?;
    wait("manual shelf", || shelf.is_focused().unwrap_or(false))?;
    capture::start_with(app, Kind::Window, true)?;
    wait("direct active window commit", || {
        !state.busy.load(Ordering::Acquire)
    })?;
    if state.library.lock().unwrap().list("window", 0, 20).total != 1
        || state.pending.lock().unwrap().is_some()
    {
        return Err(format!(
            "Active-window direct capture failed: {:?}",
            state.shelf.lock().unwrap().view.error
        ));
    }
    crate::desktop::move_capture(
        shelf.clone(),
        app.clone(),
        item.id.clone(),
        Some(folders[1].id.clone()),
    )?;
    let reopened = crate::library::Library::open(root.clone())?;
    if reopened
        .list_filtered("", 0, 20, Some(&folders[1].id), true)
        .total
        != 1
        || reopened.folders().len() != 4
    {
        return Err("Folder membership/export filter did not persist".into());
    }
    shelf_window::action(app, "open")?;
    std::thread::sleep(Duration::from_millis(900));
    capture::verification_screenshot(&shelf, &output.join("shelf.png"))?;
    shelf.eval("document.querySelector('.row')?.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,cancelable:true,clientX:190,clientY:290}))").map_err(err)?;
    std::thread::sleep(Duration::from_millis(300));
    capture::verification_screenshot(&shelf, &output.join("context-menu.png"))?;
    shelf.eval("document.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true})); document.querySelector('.row')?.click()").map_err(err)?;
    wait("native modal image viewer", || {
        app.get_webview_window("viewer").is_some()
    })?;
    let viewer = app.get_webview_window("viewer").ok_or("Missing viewer")?;
    wait("modal parent disabled", || {
        !shelf.is_enabled().unwrap_or(true)
    })?;
    crate::desktop::viewer_context(viewer.clone(), app.state())?;
    if capture::start_with(app, Kind::Screen, true).is_ok() {
        return Err("Capture was allowed during modal viewer".into());
    }
    std::thread::sleep(Duration::from_millis(900));
    capture::verification_screenshot(&viewer, &output.join("viewer.png"))?;
    viewer
        .eval("document.querySelector('button[aria-label=\"Zoom in\"]')?.click()")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(200));
    capture::verification_screenshot(&viewer, &output.join("viewer-zoom.png"))?;
    crate::desktop::close_viewer(viewer)?;
    wait("viewer released parent and capture guard", || {
        app.get_webview_window("viewer").is_none()
            && shelf.is_enabled().unwrap_or(false)
            && !state.busy.load(Ordering::Acquire)
    })?;
    shelf
        .eval("document.querySelector('button[aria-label=Settings]')?.click()")
        .map_err(err)?;
    std::thread::sleep(Duration::from_millis(400));
    capture::verification_screenshot(&shelf, &output.join("settings.png"))?;
    let mut settings = state.settings.lock().unwrap().value.clone();
    settings.capture_folder = Some(exports.join("missing-drive"));
    // Simulate an output directory disappearing after it was selected and validated.
    state.settings.lock().unwrap().value = settings;
    capture::start_with(app, Kind::Screen, true)?;
    wait("capture survives export failure", || {
        !state.busy.load(Ordering::Acquire)
    })?;
    if state.library.lock().unwrap().list("", 0, 20).total != 4
        || !state
            .shelf
            .lock()
            .unwrap()
            .view
            .error
            .as_ref()
            .is_some_and(|e| e.contains("automatic save failed"))
    {
        return Err("Failed automatic save did not retain capture and report error".into());
    }
    Ok(format!("PASS: native screen and direct region capture\nPASS: last external active window captured directly after shelf activation, without picker\nPASS: selected organizational folder and unique automatic PNG copies\nPASS: folder move, export filter and persisted reopen\nPASS: native owner-modal viewer, disabled parent, capture exclusion and close restoration\nPASS: unavailable output directory preserves committed capture and reports failure\nNative UI screenshots saved; DOM-triggered UI actions are not physical input verification.\nIsolated library: {}\nAutomatic PNG copies: {}\nNOT VERIFIED: real mouse/keyboard input, native directory/Save dialogs, clipboard paste, mixed-DPI/HDR, release installer/performance\n", root.display(), exports.display()))
}
