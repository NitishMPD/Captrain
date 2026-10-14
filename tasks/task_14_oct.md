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
