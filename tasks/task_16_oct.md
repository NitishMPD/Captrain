# Tasks - 16 October 2026

## Actual names for saved links

- Capture browser document titles (window-title fallback for address-bar capture) and derive Explorer folder names. Persist normalized, bounded names atomically in the existing title metadata; shelf rendering and search already consume this field.
- Use sanitized readable shortcut names for Save as and automatic export. Reserve automatic exports exclusively and suffix duplicates with (2), (3), etc.; preserve existing files, targets, manual rename behavior and ID-based private storage. No dependency or metadata schema change. Updated design.md.
- Verification: `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib` passed (30 passed, 2 live tests ignored), including captured-name persistence, folder naming, search, reserved/invalid Windows filenames, long titles, duplicate exports and target preservation.
- Not checked: live browser title capture or native Save dialogs. No app/installer build, push or deployment.

## Select mode and batch actions

- Added Select mode to capture context menus, with checked list/grid rows, selected count, Select visible, Actions and Done. Row clicks toggle selection; Escape exits it. Filter changes or leaving the shelf clear selection; Show more extends selectable items.
- Reused existing native move and delete commands with one delete confirmation, serialized submissions, per-item failure reporting and unfinished-item selection for retry. Permanent-folder moves keep their existing automatic-save behavior.
- Added one native batch-save command: pick a destination once, validate owned IDs, skip exported items, deduplicate IDs and reuse exclusive automatic exports. Existing files and linked targets remain protected. Updated design.md.
- Verification: TypeScript typecheck and all 37 frontend tests passed; native library tests passed (31 passed, 2 live tests ignored). Regression tests cover selection semantics, partial failures, owned-ID preflight, skipped exports, duplicate IDs and filename collisions. Browser checks using the real Shelf with mocked native commands exercised selection, skipping saved items, move, confirmed delete/cancel, failure retention, Escape and light/grid versus dark/list layout without horizontal overflow at 540px. Preview pointer actions were unreliable, so action assertions used DOM events.
- Not checked: real native folder picker, browser title accessibility, installer or OS interactions. No app/installer build, branch, push or deployment.
