# Captrain — 14 October 2026

Author: Nitish

## Completed units

- Created Tauri 2 + React/TypeScript/Vite project, npm/Cargo lockfiles, Windows icon, restricted local capabilities and design.md from PLAN.md.
- Native shelf state machine, tight native bounds, nonactivating temporary reveal, display/work-area placement, hover/hold dismissal, manual activation/deactivation, tray and single-instance handling.
- WGC screen/window acquisition, frozen-display region selector, physical coordinate validation, native transaction guard, cancellation restoration and compositor synchronization.
- Serialized local persistence, synchronized PNG/thumbnail writes, atomic index/settings replacement, malformed-file preservation, missing-file reconciliation, ID-only image/copy/delete commands and native Save as.
- Native global shortcut registration, transactional edits/defaults, duplicate/modifier validation, recorder routing and conflict feedback.
- Frontend shelf, recent list, paginated searchable library, fitted preview with copy/export/confirmed delete, settings, selection overlays, accessible buttons/focus and reduced motion.
- Used two agents total: main/native/integration GPT-6.1 Sol medium and frontend Claude Sonnet 5.5 medium.

## Verification completed

- npm run typecheck passed; 11 Vitest checks passed.
- 9 native Rust checks passed, including 1,000-record pagination, malformed-index preservation, commit/reopen/delete, coordinate bounds, overlap guard and shelf transitions. Clippy passed with warnings denied.
- Debug Tauri compiled and launched with installed Node 24.2, Rust 1.92, MSVC Build Tools 18, Windows SDKs 22621/26100 and WebView2.
- Initial real desktop checks passed: 1920×1080 WGC screen capture, foreground preserved during temporary reveal, real region capture 240×160, cancellation without history mutation, persistence reopen and native shelf/settings screenshots.
- Inspected native shelf/settings screenshots. Mechanical UI scan found only bar width/height animation; changed it to a transform inside the stable hit area.

## Final integration results / boundaries

- Final native run passed real screen/region/external-window WGC capture, foreground preservation, temporary dismissal, cancellation, persistence reopen, actual OS shortcut-conflict rollback and 100 acquisition/commit/decode/collapse cycles in 93.482 seconds. Evidence: verification/current/native-results.txt and native UI screenshots in the same directory. Diagnostic app exited successfully.
- Final full-process-tree debug sample: seven processes, 197.69 MiB private idle, 254.95 MiB after 100 cycles; 60.21-second idle CPU was 0.623% of one core / 0.0222% of this 28-logical-CPU machine. Evidence: verification/current/resources.json. Provisional 150 MB memory goal is not met in debug; release performance and long-run leak behavior remain unverified.
- Reduced persisted thumbnails to 160x100, shortened shortcut-conflict feedback, made date search tolerate punctuation, documented Rust 1.92 minimum and configured local Vite development connections. Final development compilation and native checks passed. Rapid diagnostic overlay closure sometimes logged IPC fallback warnings without failing the run.
- Real mouse/keyboard automation is blocked: Computer Use native pipe failed twice with OS error 2. Pointer/keyboard flows, clipboard paste, Save as and overwrite dialogs require manual verification.
- Mixed-DPI/negative-monitor/disconnect/HDR hardware and release performance remain unverified. Opaque-black DRM output is not reliably detectable; documented in README.
- No release build, branch, push or deployment.

## Screenshot workflow corrections completed

- Replaced the narrow first-release shelf with the supplied reference's 540x740 layout: All/Captures/Exports, list/grid, search, pastel tabbed folders and an anchored capture/folder dock. Follows the system light/dark preference; refined native scrollbar styling and light-theme hover/switch tokens.
- Organizational folder create/rename/delete, counts, filtered history, per-capture move and a persisted destination for new captures. Folder removal preserves images. Atomic combined index migrates the legacy flat history without moving originals.
- Added a native Windows directory picker for automatic PNG copies, explicit storage path in Settings, opening the destination and turning auto-save off. Exclusive unique filenames avoid replacing existing files. A missing output directory reports an error while preserving the committed library capture. Export copies survive library deletion.
- Separate native owner-modal image viewer with previous/next across paginated filtered history, fit/1:1/zoom, pointer pan, wheel/keyboard controls, folder assignment, Copy, Save as and confirmed Delete. Native parent disable/enable, focus restoration and capture/viewer exclusion protect modal lifetime.
- Custom capture/folder right-click menus, Menu key/Shift+F10, keyboard submenu navigation, viewport clamping, focus restoration, dialog focus containment and suppression of the browser page menu. Editable inputs retain text menus.
- Persisted Instant capture toggle and selected type. Instant runs the action directly; normal mode asks for the type and enables window picking. Native foreground-event tracking remembers the external active window before shelf activation. Captured thumbnails animate into the shelf after decode; reduced motion skips flight.
- Type checking passed; screenshot frontend tests 22 passed (27 with the concurrent updater tests); Rust tests 12 passed; Clippy with warnings denied passed. Added migration/folder/export failure tests and viewer transform/deep-pagination tests.
- Focused native diagnostic passed: screen/direct-region capture, direct previous-active window after shelf activation without a picker, unique auto-saved PNGs, folder/filter persistence, owner-modal lifetime and capture exclusion, and retained capture on simulated unavailable output directory. Actual native shelf/context-menu/viewer screenshots inspected. Evidence: verification/screenshot-update-final/native-results.txt and PNGs. First attempt lacked a running Vite server and timed out; fresh runs after restarting it passed.
- New diagnostic output is gitignored because captures can contain desktop content. The older resource sample predates the larger shelf/viewer; no new performance claim is made. Real input, native directory/Save dialogs, clipboard paste, mixed-DPI/HDR and release packaging remain manual/unverified.
- Added npm run package:windows for the configured x64 NSIS installer. README documents signing configuration and output path. No installer/release build was generated, following the standing no-release-build instruction. Video remains deferred by Nitish's explicit screenshots-first choice.
# GitHub Windows releases and in-app updates

- Matched FB_rea's main/master push release flow for Tauri: automatic versions across npm/Tauri/Cargo and lockfiles, Windows x64 NSIS packaging, generated release notes, signed updater assets, and draft validation before publication.
- Added installed-app checks after 10 seconds and every six hours, shelf notice, Settings check/download/progress/notes/restart controls, retryable errors, signature verification, and capture/viewer/persistence protection during install. Development does not fetch updates.
- Created the signing key outside the checkout at `C:\Users\Admin\.tauri\captrain.key` and configured the matching `TAURI_SIGNING_PRIVATE_KEY` GitHub repository secret. Updated design and release/setup documentation.
- Verification: TypeScript passed; all 27 current Vitest checks passed, including five update lifecycle checks. Three Node release-file checks passed for LF/CRLF synchronization and rejecting inconsistent versions before writing; automatic-version assertions passed. Rust updater API and Tauri action inputs were checked against their sources; no native compilation, release build, installer/update execution, push or deployment was performed. Windows Authenticode is not configured.

## App branding

- Created a canonical SVG mark matching the existing muted-blue, warm-white, rounded utility style: C-shaped capture frame with a saved-image tile.
- Applied it to the shelf's Show everything button, Settings wordmark, favicon, native tray and Windows bundle icons. The tray now uses the shared generated PNG instead of a separately drawn symbol. Kept the name, application identifier, storage paths and existing interactions.
- Updated design.md and generated only the retained Windows assets with the installed Tauri icon tool; no new dependencies.
- Verification: TypeScript passed; the design detector returned no findings. Inspected the rendered icon; asserted PNG dimensions (32, 128 and 256 pixels) and validated the six-size ICO header. No builds, native runtime checks, branches, pushes or deployment. To verify packaging locally, run `npm run package:windows`.

### Branding reference correction

- Replaced the initial capture-frame mark with Nitish's requested rounded cube silhouette and a bold perspective C on its right face. Used the existing muted blue and warm white, with a transparent background.
- Regenerated all retained Windows icons from the shared SVG, automatically updating the existing shelf, Settings, favicon and native tray consumers. Updated the brand specification in design.md.
- Verification: visually inspected 128px and 32px renders; PNG dimensions and the multi-size ICO header passed. No builds or native runtime checks; existing native binaries need rebuilding to show the updated embedded icons.

### Approved C icon

- Replaced only the shared SVG and generated Windows icon assets with vector contours following Nitish's final C image, including its soft rounded letter terminals. Retained the app's blue/warm-white colors and a transparent background; no additional UI changes.
- Updated the brand description. Inspected the generated render and checked PNG dimensions. No builds or native runtime checks.
