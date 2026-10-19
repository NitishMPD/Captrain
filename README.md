<p align="center">
  <img src="public/captrain.svg" alt="Captrain logo" width="128" height="128">
</p>

# Captrain

A Windows capture utility that lives on the edge of the screen. Click the bar to open your shelf, or capture from another application with a global shortcut. A committed capture reveals the shelf for three seconds without activating it. Hover pauses dismissal; clicking inside keeps it open.

## Run locally

Windows 11 x64, Node.js 22.12+ (or 24+), Rust 1.92+ with the MSVC toolchain, Visual Studio Build Tools with **Desktop development with C++**, a Windows 10/11 SDK, and Microsoft Edge WebView2 Runtime are required. Install Rust from https://rustup.rs and the build tools from https://visualstudio.microsoft.com/visual-cpp-build-tools/. WebView2: https://developer.microsoft.com/microsoft-edge/webview2/.

```powershell
npm ci
npm run tauri -- dev
```

Closing the panel collapses it; use the tray menu to exit. Launching a second instance opens the existing shelf. Settings select the display, left/right edge and shortcut combinations.

Before launching an installed copy, exit any development copy using **Exit Captrain** in its tray menu. Development and installed copies share the single-instance guard: a running debug copy receives subsequent launches, keeps its embedded older icon, and cannot render the shelf after its Vite server stops. Exit that copy and launch Captrain from the Start menu to use the installed app.

| Action | Default shortcut |
| --- | --- |
| Capture region | Ctrl+Alt+1 |
| Capture window | Ctrl+Alt+2 |
| Capture screen | Ctrl+Alt+3 |
| Toggle shelf | Ctrl+Alt+Space |

Screen and region capture target the display containing the pointer. Drag in either direction for a region; Escape cancels selection. Region selection stays on one display. **Instant capture** is on by default: the main Capture button executes the type selected through its separate arrow. Region opens selection immediately; Window captures the external window that was active before opening Captrain. The global window shortcut follows this setting too. Turn Instant off to make the main button ask for a capture type and use the highlighted-window picker. Window capture uses Windows Graphics Capture, including when another window overlaps it. Captrain, minimized, hidden and cloaked windows are excluded.

The reference-style shelf has All/Captures/Exports tabs, list/grid layout, search, colored folders and a capture/folder dock. It starts with 20 captures; Show more loads additional pages. Search matches kind, filename and local date. New folder creates an organizational folder; its context menu supports rename and deletion. Deleting a folder leaves its captures in the library. The footer's Folder selects where new captures are organized. A capture's right-click menu (also Menu key or Shift+F10) offers Open, Copy, Save as, Move to folder and confirmed Delete. Text fields retain their native editing menu.

Click an image to open the native modal viewer over the shelf. Previous/next navigate the filtered library, including beyond the shelf's loaded page. Wheel or +/− zoom, drag pans, 0 fits, 1 shows actual size, arrows navigate, and Escape closes. The viewer also provides Copy image (Ctrl+C), native Save as (Ctrl+Shift+S), folder assignment and confirmed Delete. A new capture animates into the shelf; reduced motion skips the flight. The interface follows the system's light/dark preference. Captures never automatically replace the clipboard. Selecting a shortcut button records a new combination; registered combinations are routed into the recorder instead of triggering a capture. Failed registrations retain the previous working bindings.

## Data and recovery

Data is local to `%LOCALAPPDATA%\com.nitish.captrain\`, independent of this checkout:

```text
captures/<uuid>.png
thumbnails/<uuid>.png
index.json
settings.json
```

**Choose a Windows save folder:** open **Settings → Save to disk → Choose folder**, or the footer's **Folder → Choose save location**. New captures are automatically copied there with unique PNG names. Turning automatic save off keeps new captures in the protected library only. Settings displays the protected library's exact path. Existing captures are not moved when changing the destination. If a drive disconnects or saving fails, the committed library capture is retained and an error is shown. Deleting a capture from Captrain removes its owned original and thumbnail; exported copies remain on disk.

Organizational folders and memberships are persisted in `index.json`. Existing flat histories load without data movement and migrate to the combined items/folders index on their next mutation.

Rust serializes mutations, writes and synchronizes images before replacing the index atomically, and emits capture-created only after commit. File commands resolve IDs inside the owned library; frontend content has no generic filesystem or shell access. Save as exports to a destination chosen in the native dialog, with its overwrite prompt, and leaves the library original intact. Exporting into the private library is rejected.

Malformed indexes/settings are preserved and reported, and their writes are blocked. To recover, exit Captrain, preserve the damaged file elsewhere, then restore a valid copy of that file. Removing a damaged index starts an empty history and leaves image files on disk; it does not import orphaned images. Missing originals are hidden from results while their metadata remains. There is no automatic purge, telemetry or remote UI content. Cloud upload is opt-in through Settings ? Cloud backup.

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

Screenshot-update checks passed: 12 Rust tests, frontend tests (22 screenshot tests plus 5 concurrent updater tests), type checking and Clippy. The new native evidence is in `verification/screenshot-update-final/native-results.txt` and screenshots in that directory. It verifies direct screen/region capture, the external active window after shelf activation, unique auto-saved copies, folder persistence/filtering, native owner-modal open/close and preservation of a capture when the output directory disappears. The UI actions in this diagnostic use DOM events, not real pointer/keyboard input. To repeat this focused diagnostic, set `CAPTRAIN_VERIFY_UPDATE=1` alongside `CAPTRAIN_VERIFY` before starting development mode, and remove both afterward. Diagnostic images are gitignored because they may include desktop content.

Earlier checks in `verification/current/native-results.txt` and `verification/current/resources.json` passed real screen, region and external-window acquisition, cancellation, persistence, shortcut-conflict rollback and 100 capture cycles (93.482 seconds). See `tasks/task_14_oct.md` for the record. The current environment has Windows 11 Pro build 26200, Intel Core i7-14700, Rust 1.92, Node 24.2, MSVC Build Tools 18 and SDKs 22621/26100. Only a 1920×1080 display at 100% scale has been exercised. The native UI screenshots are real Tauri/WebView2 output.

The earlier debug sample included Captrain and its six WebView2 processes: 197.69 MiB private memory idle, 254.95 MiB after 100 cycles, and 0.623% of one CPU core (0.0222% of this 28-logical-CPU machine) over 60.21 idle seconds. It predates the larger shelf and separate viewer. Debug memory exceeded the provisional 150 MB goal; resource use has not been remeasured for the new UI. This bounded run does not establish leak-free behavior or release resource use. Some rapidly closed diagnostic overlays logged Tauri IPC transport fallback warnings; the native checks completed successfully. Release latency and memory acceptance remain open.

The Computer Use native pipe is unavailable here, so real keyboard/pointer interaction, clipboard paste into another app and native Save as interaction require manual checks. Test the shortcuts from another app, reverse region selection, window picking under overlap, Escape, hover pause, outside-click dismissal, image Copy/paste, export/overwrite/cancel, confirmed deletion and relaunch. Also test display disconnect and monitors at negative positions with 100%, 150% and 200% scaling, and a small work area. Check keyboard-only navigation, reduced motion, shortcut-recording conflicts and tray reopen/exit.

WGC access failures, explicit window display-affinity protection, unavailable targets, timeouts and fully transparent frames produce errors. Some DRM applications return opaque black pixels without identifying their protection; Captrain cannot reliably distinguish those from a legitimate dark image. This remains a limitation: inspect captures from protected applications. No desktop crop is substituted for a failed window capture. Secure desktop/UAC and exclusive fullscreen are outside the supported capture surface.

Output is 8-bit SDR PNG. HDR tone/color accuracy is unverified and may clip highlights; validate on HDR hardware before relying on color fidelity. The default Windows capture border is retained. Development timing/memory does not establish release performance; the plan's latency, CPU and memory figures are provisional goals.

Browser link saving, appearance, Windows startup, organizational folders and initial video recording are implemented. Tags, annotation, OCR, full two-way sync and alternate screenshot formats remain deferred. Google Drive backup and additive restore are available through Settings ? Cloud backup; see [setup instructions](docs/google-drive-setup.md).

## Recording

Use **Record video** in the footer. Screen, window and region recording default to **silent screen only**. System sound, a selected microphone and a selected webcam are independent opt-ins. Setup offers 30/60 fps, three quality presets, original/1080p/720p output, cursor visibility and a countdown. Open the red shelf bar for Pause/Resume and Stop. After stopping, play the MP4 with Windows' default video player, save a copy, move it to a library folder or delete it. Automatic disk copies also support MP4. The webcam currently uses a square inset, with corner and size choices; circular masking and mirror are not implemented.

The native recording helper uses ScreenRecorderLib 7.0.1 (MIT), Windows Graphics Capture, D3D11, WASAPI and Media Foundation. It runs only during device enumeration or recording; frames never pass through the web UI. Windows may select software despite the hardware-encoding request, so hardware selection and smoothness must be verified on each supported GPU. No frame-drop, HDR, audio sync, webcam or sustained performance claim has been verified by the automated storage/UI checks.

For development, install the .NET SDK and existing Visual Studio C++ Build Tools, then run:

```powershell
npm run recorder:build
npm run tauri -- dev
```

The recording helper uses Windows 11's .NET Framework 4.8. The build script bundles app-local VC runtime DLLs and the recorder license. `npm run package:windows` builds and bundles the helper automatically. These commands are provided for you to run; no app/helper/installer binary was built during this change.

To check C# API compatibility **without emitting a binary**, restore packages and run:

```powershell
dotnet restore recorder/Captrain.Recorder.csproj -p:Platform=x64 --locked-mode
powershell -NoProfile -File recorder/typecheck.ps1
```

Interrupted/unindexed MP4s are retained under the private storage directory's `recordings` folder. Errors and the next launch report their location. Copy them elsewhere before attempting repair; recovery is manual and damaged files are not guaranteed playable. Finish recording before exiting Captrain or installing an update. Windows N/KN requires the Media Feature Pack.

## Release build (run yourself)

### Automatic GitHub releases

Like FB_rea, pushing to `main` or `master` runs `.github/workflows/release.yml` on Windows, tests the app, builds an x64 NSIS installer, and publishes a GitHub Release with generated notes. No manual tag is needed. CI uses the checked-in base version plus `GITHUB_RUN_NUMBER` for the patch (for example, base `0.1.0` and run 10 become `0.1.10`). Keep the base versions in `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and their lockfiles aligned. Version changes stay in CI; nothing is committed back. Keep major/minor at most 255 and the resulting patch at most 65535 for Windows installer compatibility.

The release stays a draft until the installer, `.exe.sig`, and signed-update manifest `latest.json` have uploaded and passed asset checks. A failed run may leave a draft; remove that failed draft/tag before rerunning the same run number. This workflow builds and publishes only when you push; no push or release was performed during setup.

The public feed is `https://github.com/NitishMPD/Captrain/releases/latest/download/latest.json`. Install the first release manually. Installed release apps check after 10 seconds and every six hours. **Settings > App updates** supports manual checks, download progress, release notes, and **Restart and install**. Downloads and restarts require your action. The shelf shows an update notice. Finish any capture and close the viewer before installation. Your library and settings remain in local app data. Development mode does not fetch updates; failed downloads and installations can be retried.

Tauri requires signed updates. The matching `TAURI_SIGNING_PRIVATE_KEY` repository secret has been configured on `NitishMPD/Captrain`; its public key is in `src-tauri/tauri.conf.json`. The private key is stored outside this checkout at `C:\Users\Admin\.tauri\captrain.key`. Back it up securely: replacing it breaks updates for existing installations. The generated key has no password, so leave `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` unset. For a future protected replacement key, configure both secrets and plan key migration before shipping. Update signing is separate from Windows Authenticode; Windows may display an unknown-publisher/SmartScreen prompt.

Focused update checks, without building:

```powershell
node scripts/release-version.cjs --check
node --test scripts/release-version.check.cjs
npx vitest run src/updates/controller.test.ts
```

### Manual local installer

For a signed local installer, set the private-key path before building. Do not commit the key:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = 'C:\Users\Admin\.tauri\captrain.key'
npm run package:windows
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY
```

The Windows x64 installer is produced at `src-tauri/target/release/bundle/nsis/Captrain_0.1.0_x64-setup.exe` for the checked-in version. No installer exists until that command completes; installer generation was not run here under the standing no-release-build instruction.

No release build, branch, push or deployment was performed. The source icon and tray mark are simple procedural geometry authored for this app; there are no generated photographic assets or remote fonts.

## Google Drive backup

Settings ? Cloud backup connects your own Google account using a Desktop OAuth JSON imported locally. Enable the Google Drive API and create the OAuth client first; follow [Google Drive setup](docs/google-drive-setup.md). The example configuration is in [cloud/oauth-client.example.json](cloud/oauth-client.example.json); save the real downloaded file as gitignored `cloud/oauth-client.json` and import it through Settings. No Captrain backend or storage subscription is required; Google's account storage and API limits apply.

Choose screenshots, web links and optionally videos. Existing history is opt-in. While Captrain runs, selected changes back up within the next minute; pause and manual backup are available. Restore missing items adds cloud items without overwriting local entries. Local deletion and disconnect keep cloud copies. This release provides backup/restore rather than automatic two-way sync. Local paths, Windows folder links and device settings are excluded. Credentials are encrypted for the current Windows user. No live Google account was connected during source verification.
