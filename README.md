# Captrain

A Windows capture utility that lives on the edge of the screen. Click the bar to open your shelf, or capture from another application with a global shortcut. A committed capture reveals the shelf for three seconds without activating it. Hover pauses dismissal; clicking inside keeps it open.

## Run locally

Windows 11 x64, Node.js 22.12+ (or 24+), Rust 1.92+ with the MSVC toolchain, Visual Studio Build Tools with **Desktop development with C++**, a Windows 10/11 SDK, and Microsoft Edge WebView2 Runtime are required. Install Rust from https://rustup.rs and the build tools from https://visualstudio.microsoft.com/visual-cpp-build-tools/. WebView2: https://developer.microsoft.com/microsoft-edge/webview2/.

```powershell
npm ci
npm run tauri -- dev
```

Closing the panel collapses it; use the tray menu to exit. Launching a second instance opens the existing shelf. Settings select the display, left/right edge and shortcut combinations.

| Action | Default shortcut |
| --- | --- |
| Capture region | Ctrl+Alt+1 |
| Capture window | Ctrl+Alt+2 |
| Capture screen | Ctrl+Alt+3 |
| Toggle shelf | Ctrl+Alt+Space |

Screen and region capture target the display containing the pointer. Drag in either direction for a region; click a highlighted window in window mode. Escape cancels selection. Region selection stays on one display. Window capture uses Windows Graphics Capture of the selected window, including when another window overlaps it. Captrain, minimized, hidden and cloaked windows are excluded.

Recent shows the newest 20 captures. Library searches kind, filename and local date. Preview supports Copy image (Ctrl+C), native Save as (Ctrl+Shift+S), and confirmed Delete. Captures never automatically replace the clipboard. Selecting a shortcut button records a new combination; registered combinations are routed into the recorder instead of triggering a capture. Failed registrations retain the previous working bindings.

## Data and recovery

Data is local to `%LOCALAPPDATA%\com.nitish.captrain\`, independent of this checkout:

```text
captures/<uuid>.png
thumbnails/<uuid>.png
index.json
settings.json
```

Rust serializes mutations, writes and synchronizes images before replacing the index atomically, and emits capture-created only after commit. File commands resolve IDs inside the owned library; frontend content has no generic filesystem or shell access. Save as exports to a destination chosen in the native dialog, with its overwrite prompt, and leaves the library original intact. Exporting into the private library is rejected.

Malformed indexes/settings are preserved and reported, and their writes are blocked. To recover, exit Captrain, preserve the damaged file elsewhere, then restore a valid copy of that file. Removing a damaged index starts an empty history and leaves image files on disk; it does not import orphaned images. Missing originals are hidden from results while their metadata remains. There is no automatic purge, upload, telemetry or remote UI content.

## Focused checks

```powershell
npm run typecheck
npx vitest run
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings
```

The opt-in **debug-only** native verification creates a separate temporary library. It runs real WGC acquisition, selection/cancellation, native shortcut-conflict rollback, persistence reopening and 100 acquisition/commit/decode/collapse cycles, saves native shelf/settings screenshots and exits. It briefly opens selection overlays and activates the shelf during the check. Close other Captrain instances first. Use a fresh output directory on each run; this is a diagnostic check, not a production mode.

```powershell
$env:CAPTRAIN_VERIFY = Join-Path $PWD 'verification/new-run'
npm run tauri -- dev
Remove-Item Env:CAPTRAIN_VERIFY
```

The chosen output directory's `native-results.txt` records the result and location of the isolated capture data. Those files are intentionally retained for inspection. Delete that explicitly named temporary directory when finished. The native check invokes selection commands directly; it does not establish real pointer or keyboard input behavior.

For a 60-second idle CPU/private-memory sample of Captrain and its WebView2 process tree, run this in another terminal alongside verification, using the same fresh directory:

```powershell
powershell -NoProfile -File scripts/measure-dev.ps1 -OutputDirectory verification/new-run
```

The implementation keeps shelf state and the single replaceable dismissal deadline in Rust, next to native focus/capture ownership. Deterministic Rust tests cover timer replacement, hover/hold pause, manual persistence and cancellation restoration. TypeScript checks cover reverse dragging, physical coordinate conversion and shortcut recording. Full-resolution buffers live only through acquisition/commit or preview/copy; visible pages load small thumbnails.

## Verification limits

See `verification/current/native-results.txt`, `verification/current/resources.json`, and `tasks/task_14_oct.md` for actual results. All 11 frontend and 9 Rust tests, type checking and Clippy passed. Native checks passed for real screen, region and external-window acquisition, cancellation, persistence, shortcut-conflict rollback and 100 capture cycles (93.482 seconds). The current environment has Windows 11 Pro build 26200, Intel Core i7-14700, Rust 1.92, Node 24.2, MSVC Build Tools 18 and SDKs 22621/26100. Only a 1920×1080 display at 100% scale has been exercised. The native UI screenshots are real Tauri/WebView2 output.

The final debug sample included Captrain and its six WebView2 processes: 197.69 MiB private memory idle, 254.95 MiB after 100 cycles, and 0.623% of one CPU core (0.0222% of this 28-logical-CPU machine) over 60.21 idle seconds. Debug memory exceeds the provisional 150 MB goal. This bounded run does not establish leak-free behavior or release resource use. Some rapidly closed diagnostic overlays logged Tauri IPC transport fallback warnings; the native checks completed successfully. Release latency and memory acceptance remain open.

The Computer Use native pipe is unavailable here, so real keyboard/pointer interaction, clipboard paste into another app and native Save as interaction require manual checks. Test the shortcuts from another app, reverse region selection, window picking under overlap, Escape, hover pause, outside-click dismissal, image Copy/paste, export/overwrite/cancel, confirmed deletion and relaunch. Also test display disconnect and monitors at negative positions with 100%, 150% and 200% scaling, and a small work area. Check keyboard-only navigation, reduced motion, shortcut-recording conflicts and tray reopen/exit.

WGC access failures, explicit window display-affinity protection, unavailable targets, timeouts and fully transparent frames produce errors. Some DRM applications return opaque black pixels without identifying their protection; Captrain cannot reliably distinguish those from a legitimate dark image. This remains a limitation: inspect captures from protected applications. No desktop crop is substituted for a failed window capture. Secure desktop/UAC and exclusive fullscreen are outside the supported capture surface.

Output is 8-bit SDR PNG. HDR tone/color accuracy is unverified and may clip highlights; validate on HDR hardware before relying on color fidelity. The default Windows capture border is retained. Development timing/memory does not establish release performance; the plan's latency, CPU and memory figures are provisional goals.

Browser links, video, startup launch, folders/tags, annotation, OCR, sync and alternate formats are deferred as specified in PLAN.md.

## Release build (run yourself)

```powershell
npm run tauri -- build
```

No release build, branch, push or deployment was performed. The source icon and tray mark are simple procedural geometry authored for this app; there are no generated photographic assets or remote fonts.
