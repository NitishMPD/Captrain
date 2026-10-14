# Captrain implementation plan

Author: Nitish  
Date: 2026-10-14  
Status: First-release implementation complete. Native integration checks pass; manual input, hardware coverage and release-performance acceptance remain pending. See README.md and tasks/task_14_oct.md.

> Agent handoff: Read this document and applicable AGENTS.md instructions, inspect the repository, then implement the first release task by task. Finish the real desktop capture flow. Do not stop at scaffolding or a browser-only mockup. No subagents unless Nitish explicitly requests them.

**Goal:** Build a small Windows capture utility that normally occupies only an edge bar. Captures briefly appear in a floating shelf, like adding things to a shelf, then the shelf goes away.

**Architecture:** Tauri 2 renders a React/TypeScript interface. Rust owns Windows capture, native window behavior, global shortcuts, clipboard operations, and persistence. One native process owns capture transactions and shortcut registration; the UI consumes committed results.

**Tech stack:** Tauri 2, React, TypeScript, Vite, Rust, CSS, maintained Windows bindings. Use official Tauri plugins for capabilities they already own and a maintained encoder for PNG/thumbnails. Avoid a UI framework, ORM, state library, or animation library unless a demonstrated need justifies it.

## 1. Implementation instructions

- Windows only. Proposed baseline: Windows 11 x64. Record any narrower API requirement before relying on it.
- UX first. Shelf behavior and a polished UI are acceptance requirements.
- Follow existing design.md. If absent, create it with author Nitish during implementation, using this plan as its source. Updating design is part of implementation, not another approval step.
- This standalone planning handoff intentionally creates no design or task files. During implementation, update only tasks/task_<day>_<month>.md after a meaningful unit is complete. Do not create duplicate task files or read backups unless asked.
- Never run builds. Supply exact build commands for Nitish. Focused tests/static checks may run; Rust tests compile test targets, so provide cargo test for Nitish if running it conflicts with the no-build instruction.
- Never push, deploy, or create a branch without explicit authorization.
- No accounts, telemetry, remote UI content, or uploads.
- Preserve committed captures across restart. Report errors rather than pretending capture succeeded.
- Browser checks cannot prove native focus, shortcuts, capture, or DPI behavior. Report what remains unverified.

## 2. Scope and reversible defaults

Confirmed requirements: full-screen, window and rectangular marquee capture; collapsed edge bar; scale on hover; click to open; temporary reveal on capture; customizable keyboard shortcuts; modern UI; small footprint and strong performance. Active-browser link capture and video are part of the product direction.

First release: still captures, shelf behavior, recent history/library, image preview, Copy, Save as, Delete, shortcut customization, minimal settings, tray reopen/exit.

Later stages: browser links, then video. Do not ship fake or disabled Record controls.

Proposed defaults, not additional user requirements:

- Right edge of the primary display; settings select left/right and target display.
- Screen capture means the display containing the pointer when the command starts.
- Window capture highlights eligible windows under the pointer; click confirms.
- Region selection stays within one display in the first release.
- Temporary reveal lasts 3 seconds unless interacted with.
- Copy is explicit; captures do not automatically overwrite the clipboard.
- Defer folders, tags, annotation, OCR, sync, startup launch and export-format choices.

## 3. Shelf behavior contract

Use explicit modes: collapsed, temporary preview, manual open, capturing. Hover is a visual condition. One transition controller owns dismissal; do not scatter timers through components.

- Launch collapsed.
- Hover enlarges the bar inward. Keep it inside a stable hit area to avoid flicker. Hover alone never opens it.
- Clicking the bar or Toggle shelf opens manually and permits keyboard focus.
- Manual open remains until Escape, an outside click, or Toggle shelf.
- A successfully persisted capture reveals the shelf without activating its window or taking foreground keyboard focus.
- A capture while manually open inserts the item and leaves it manually open.
- A capture while collapsed/temporarily open starts or restarts a 3-second timer after the item becomes visible.
- Hover pauses temporary dismissal. Leaving starts a fresh 3-second timer.
- Clicking inside a temporary preview promotes it to manual open. Menus, settings and native dialogs prevent automatic dismissal.
- Old timers cannot collapse a newer reveal or a manually opened shelf.
- Escape cancels a capture overlay first; otherwise it collapses the shelf.
- Hide app surfaces before acquiring pixels and wait for compositor updates. A fixed arbitrary delay is not proof that the shelf is absent.
- Cancellation restores the previous shelf mode and creates no history item.
- Permit one native capture transaction at a time; repeated shortcuts do not create overlapping overlays.
- Failures restore usable UI and show a readable error without stealing focus.
- Invisible areas around the collapsed bar must not intercept clicks intended for other apps.
- Keep the shelf above normal application windows. Do not attempt to overlay UAC/secure desktop; exclusive fullscreen is a test boundary.
- Closing the shelf collapses it. Exit is explicit in the tray; exit releases overlays and shortcuts.

Test timer replacement, hover pause, manual-open persistence, cancellation restoration and repeated capture requests deterministically. The native transaction guard must not depend on a React UI state.

## 4. Visual specification

Reference: [opened floating shelf](references/shelf-reference.png).

Borrow warm white surfaces, generous spacing, rounded corners, subtle borders, recognizable thumbnails and anchored bottom controls. The orange background is not app UI. Omit the reference's macOS shortcut glyphs, folder cards, export tabs and filename-dominated rows.

Starting dimensions in logical pixels:

- Bar: visible 6 x 56 within a 16 x 72 clickable area; hover grows inward to roughly 9 x 64.
- Shelf: roughly 360 wide by 520 high, inset 10 from the screen edge, clamped to the work area.
- Outer radius 18; item radius 10; base spacing 8; normal padding 16.
- Segoe UI Variable with Segoe UI fallback; 13-14 px readable body text.
- Warm near-white surface, charcoal text, muted blue accent, subtle gray borders.
- Restrained 140-180 ms opacity/small-translation transitions. Respect reduced motion. No continuous animation or expensive backdrop blur.

Layout: header with Captrain/search/settings; scrollable recent captures with thumbnail, kind, readable time and dimensions; anchored Capture button with Region/Window/Screen menu and Library action.

Recent shows the newest 20 captures. Library uses the same panel and data with paginated search over kind, displayed date and filename. Selecting an item opens a fitted preview with Copy, Save as, Delete and Back. Delete requires confirmation; Save as uses the native overwrite prompt.

Required states: collapsed/hover; recent empty/populated; newest-item reveal; capture menu; preview; library; no search results; settings; shortcut conflict; capture error; region/window overlays.

Use actual buttons, icon labels, visible focus rings, keyboard navigation and sufficient contrast. Essential actions must work without hover. A DOM key handler does not qualify as a global shortcut.

## 5. Native capture and trust boundaries

Resolve exact APIs from current Windows/Tauri/plugin documentation while implementing; do not invent SDK signatures.

- Record foreground application and target display before Captrain changes focus.
- Capture uses physical pixel coordinates. Explicitly convert logical overlay coordinates, including mixed DPI and monitors left/above the primary display.
- Freeze a clean display snapshot before region overlays. Reverse dragging works; reject zero-area selections.
- Screen output matches the selected display's native pixel dimensions.
- Window picker excludes Captrain, minimized/invisible windows and unavailable targets.
- Use native window capture so overlapping windows are not silently included. Prefer Windows Graphics Capture where appropriate; verify its border/access behavior on the supported Windows version.
- Protected/unsupported content must fail clearly. Do not advertise success for a black image or silently substitute a desktop crop. Legitimate dark content must not be rejected merely because it is dark.
- Test HDR/color behavior and produce viewable SDR PNGs; document any remaining limit.
- Acquire/encode off the UI thread and release full-resolution buffers after commit.
- Prefer native messages/events for hover, activation, outside clicks and display changes. Avoid polling/global mouse hooks unless a verified gap needs them.
- Display disconnect/reconfiguration moves the shelf into an available work area.
- Copy puts image data on the clipboard, not a path.
- Tray offers Open shelf and Exit. A second instance opens the existing shelf instead of registering shortcuts twice.
- Restrict Tauri capabilities to necessary local operations. No arbitrary filesystem/shell commands from web content. Rust receives capture IDs and resolves owned files; a native Save as dialog authorizes export destinations.

## 6. Persistence and shortcut ownership

Use the platform-provided per-user local app-data directory, independent of the checkout. Suggested layout: captures/<id>.png, thumbnails/<id>.png, index.json and settings.json. IDs must be collision resistant; timestamps alone are insufficient.

Metadata: id, capture kind, UTC creation time, physical width/height, image filename and thumbnail filename. Localize date labels in the UI. Do not collect window titles or private paths unnecessarily.

Write image and thumbnail before atomically replacing the metadata index. Emit capture-created only after commit. Serialize mutations to prevent lost updates. Preserve malformed metadata/settings instead of overwriting them silently. Reconcile missing files safely. Failed export leaves the original intact. Do not silently purge captures.

Load metadata and small thumbnails only for the visible page. Decode originals for preview/copy/export; release unused buffers/object URLs.

Proposed global shortcuts:

- Ctrl+Alt+1: region capture.
- Ctrl+Alt+2: window capture.
- Ctrl+Alt+3: screen capture.
- Ctrl+Alt+Space: toggle shelf.

When a preview has focus: Ctrl+C copies; Ctrl+Shift+S opens Save as; Delete requests deletion.

Shortcut editing records combinations, rejects unmodified letter/number globals, detects internal duplicates and checks actual native registration conflicts. On failure retain the previous working binding, report the conflict and do not persist the failed edit. Default restoration follows the same rules and cannot silently disable working bindings. Menus show current bindings.

## 7. Intended file responsibilities

Adapt names to the generated scaffold; do not create empty scaffolding merely to match this list.

- package.json, vite.config.ts, tsconfig.json: frontend configuration and scripts.
- src-tauri/Cargo.toml, tauri.conf.json, capabilities/default.json: native dependencies/window/security configuration.
- src-tauri/src/lib.rs: setup and narrow command/event wiring.
- src-tauri/src/capture.rs: capture APIs and native transaction ownership.
- src-tauri/src/shelf_window.rs: placement, focus, hit region, display changes.
- src-tauri/src/library.rs: persistence, export and deletion.
- src-tauri/src/shortcuts.rs: registration and settings changes.
- src/App.tsx: composition.
- src/shelf/state.ts and state.test.ts: pure transitions and focused tests.
- src/shelf/Shelf.tsx, src/library/Library.tsx, src/settings/Settings.tsx: user flows.
- src/native.ts: typed commands/events; no generic transport framework.
- src/styles.css: shared visual tokens, states, layout and motion.
- README.md: prerequisites, commands, manual checks, limits.
- design.md and the one daily task file: maintained during implementation.

Keep Rust unit checks beside coordinate, persistence and shortcut logic. Keep test fixtures out of production capture data.

## 8. Implementation sequence and acceptance

### Task 1: Real desktop shell and behavior proof

- [ ] Check Rust/MSVC, Windows SDK, Node/package manager and WebView2 prerequisites. Record missing prerequisites with exact instructions.
- [ ] Scaffold Tauri 2 + React/TypeScript + Vite using supported releases; keep lockfiles.
- [ ] Create design.md from this specification.
- [ ] Implement edge placement, stable hover hit area, tight window bounds, tray and single-instance behavior.
- [ ] Implement shelf transitions with a development-only simulated capture action, never a production fake capture button.
- [ ] Fake-clock tests: preview expires at 3 seconds; manual never auto-closes; hover pauses; new reveal invalidates old timeout; clicking preview makes it manual.
- [ ] Prove native temporary reveal preserves foreground focus and collapsed transparent space does not block clicks.

Acceptance: a real Windows window follows the shelf contract. Fix native focus/hit-region limitations before adding features. If a measured hard WebView2 limitation remains, document evidence and discuss a native UI alternative before a major rewrite.

### Task 2: Complete region capture flow

- [ ] Implement clean display snapshot, selection overlay, cancellation and native transaction guard.
- [ ] Persist PNG/thumbnail/metadata; emit committed item events.
- [ ] Show the real thumbnail, temporary reveal, preview, image Copy, Save as and confirmed Delete.
- [ ] Check reverse dragging, negative desktop coordinates, DPI conversion, zero area, cancellation and persistence failure.
- [ ] Verify overlays/shelf are absent from output. Restart and confirm captures remain accessible.

Acceptance: shortcut -> selection -> stored capture -> temporary shelf -> preview/copy/export works with real pixels. Cancellation/failure leaves history unchanged.

### Task 3: Screen/window capture and shortcut settings

- [ ] Reuse the transaction and persistence for screen/window capture.
- [ ] Add window picker and native capture with explicit unavailable/protected failures.
- [ ] Register globals and route buttons/shortcuts through the same native entry point.
- [ ] Add shortcut editing, duplicate/conflict errors, rollback, persistence and default restoration.
- [ ] Check failed registration preserves old binding; concurrent capture attempts cannot overlap.
- [ ] Verify modes on multiple displays and 100%, 150%, 200% scaling where hardware permits.

Acceptance: actions work while another app is focused. Failed edits preserve working shortcuts. Invalid targets/displays fail cleanly.

### Task 4: Finished UI and library

- [ ] Implement all visual states and reference-based styling.
- [ ] Add recent history, searchable paginated library, settings and readable empty/error states.
- [ ] Check keyboard-only use, accessible labels/focus/contrast, reduced motion and small work areas.
- [ ] Exercise 1,000 metadata records without decoding all originals.
- [ ] Inspect real desktop screenshots of bar/hover, populated shelf, preview, settings/conflict and empty/error states. Use browser automation only where needed for frontend proof.
- [ ] Record native manual-check results and any hardware/prerequisite blockers.

Acceptance: coherent finished UI, no placeholders/dead controls, no hover-only essentials, no reference screenshot substituted for implementation.

### Task 5: Resource checks and handoff

- [ ] Record machine, Windows/WebView2 versions, capture dimensions and development/release mode.
- [ ] Measure whole-app CPU/private memory, including associated WebView2 processes, after 60 seconds idle collapsed.
- [ ] Measure click-to-visible and pixel-acquisition-to-thumbnail latency separately from user selection time.
- [ ] Repeat 100 capture/preview/collapse cycles to find retained buffers, listeners or timers.
- [ ] Document focused checks and actual results. Suggested frontend commands once scripts exist: npm run typecheck; npm test -- --run. Native test command for Nitish: cargo test --manifest-path src-tauri/Cargo.toml.
- [ ] Document local run command npm run tauri dev and release command npm run tauri build. Do not run builds. Native dev may compile Rust, so give Nitish the launch command when it conflicts with that restriction.
- [ ] Update the daily task file with completed work and limitations.

Provisional performance goals, not measurements or guarantees: collapsed idle CPU below 1% on the recorded machine; whole-app private memory below 150 MB after settling; warm reveal within 150 ms; thumbnail within 500 ms of pixel acquisition for a 1080p display. Measure release mode when Nitish supplies/runs it. Development measurements cannot establish release acceptance.

Stop first-release work when the complete path and focused checks pass. Report blocked checks or missed goals honestly with the smallest next action.

## 9. Later stages

### Active-browser link capture

Before implementing, agree supported browsers and whether installing a small extension is acceptable. Reliable active-tab URLs cannot be inferred from window titles. UI Automation is a possible limited alternative, requiring per-browser proof rather than claims of universal support.

A future explicit command identifies the foreground browser before shelf activation, retrieves only the intended active tab's HTTP/HTTPS URL/title, stores a link item and uses the existing shelf reveal. No continuous history collection. Validate schemes and open stored links through the OS handler. Extension messaging needs a documented trusted local channel.

### Video recording

Plan separately after still capture stabilizes. Decide recording targets, microphone/system audio, codecs, indicator, stop shortcut, disk-space errors and interrupted-file recovery. Reuse shelf presentation where appropriate; do not build a generalized media/provider framework now.

## 10. Agent completion report

Report what works, checks actually run, commands Nitish must run, changed file paths and material limitations. Never label a frontend mockup a working desktop capture app.
