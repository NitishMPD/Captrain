# Captrain design

Author: Nitish

Source of truth: PLAN.md, first release. Windows 11 x64, Tauri 2, React and TypeScript, Rust native ownership. Still captures only; links and video are later work.

## User experience

Launch as a right-edge 16 × 72 logical-pixel hit area containing a 6 × 56 bar. Hover grows the bar inward, never opens it. Click opens a warm-white 360 × 520 shelf, clamped to the monitor work area. Left/right edge and display are settings. Segoe UI Variable/Segoe UI, charcoal text, muted blue accents, rounded 18px shelf, 10px cards, 8px spacing scale, restrained motion with reduced-motion support.

Header/search/settings, recent thumbnail captures, anchored Capture menu and Library. Preview provides Copy image, Save as, confirmed Delete and Back. Library searches kind, localized date and filename with pagination. Settings record actual global shortcut combinations and report native registration conflicts without discarding working bindings. No Record placeholders, automatic clipboard replacement, accounts or remote content.

## Native invariants

Rust owns the shelf state machine and its single replaceable dismissal deadline, native placement/focus, capture transaction, shortcuts and serialized persistence. This consolidates dismissal with native focus and capture events rather than duplicating timers in React. Pure Rust state tests cover the same deterministic transition contract proposed for src/shelf/state.ts in PLAN.md. Temporary reveal uses show-without-activation and arms dismissal after frontend ready; manual open explicitly activates. Hover and holds pause dismissal; stale deadlines cannot close newer/manual state. Native deactivation collapses manual mode. Capture cancellation restores prior mode; capture completion never promotes temporary to manual. Tight native window bounds leave no large invisible click-catching surface.

One native transaction freezes a clean display after hiding app surfaces and synchronizing the compositor. Region selection uses physical pixels over the frozen display. Window selection uses eligible window rectangles and Windows Graphics Capture of the selected native window, never a desktop crop. PNGs and thumbnails commit before the atomic index replacement and notification. All file commands accept owned capture IDs. The frontend receives data URLs only for the visible page/preview. Export uses a native Save dialog and preserves the original.

All files stay in per-user local app data. Malformed metadata/settings are preserved and reported. No silent purge. Shortcut changes register new bindings transactionally and retain old bindings on conflict. Single-instance and tray reopen/exit are required.

## Verification boundary

Development compilation and native tests authorized by Nitish for this implementation. Release builds, branches, pushes and deployment remain excluded. Native visual/focus/DPI/HDR and performance results must be recorded as measured or unverified, never inferred from browser checks.
