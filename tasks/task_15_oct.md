# Tasks - 15 October 2026

## Browser URL saving, appearance and Windows startup

- Added Ctrl+Alt+4 (editable in Settings) and a shelf link button to save a page from Chrome, Edge or Brave without an extension, keystroke injection or clipboard replacement. Windows UI Automation reads visible document metadata, with an address-bar fallback; unsupported or ambiguous pages fail visibly.
- Saved HTTP/HTTPS links share library search, folders, copy, browser opening, .url export, automatic disk copies and confirmed deletion. Screenshot-only filters keep links out of image-viewer navigation. Legacy screenshot metadata and custom shortcut bindings are preserved.
- Added persisted System/Light/Dark controls using the existing palettes across the shelf and viewer. Added an opt-in Start with Windows switch backed by the current user's quoted executable Run entry; enable it from the installed copy.
- Updated design.md. No new dependencies; enabled COM and Registry features on the existing Windows crate.
- Verification: TypeScript typecheck passed; 27 frontend tests passed; 17 native tests passed. The opt-in native accessibility test also passed against a live Chrome window without focusing its address bar. Browser UI checks with mocked native commands covered link open/save actions, light/dark changes, startup state and no horizontal overflow at the native 540x740 size. Impeccable detector returned no findings.
- Not verified: Edge/Brave accessibility behavior, real global shortcut input, native link-opening/clipboard/Save dialogs, actual sign-in startup or installer behavior. No app/installer build, push or deployment. The existing installed app must be rebuilt and installed to receive these changes.

## Link dropdown and saved-link presentation

- Moved Save browser link (Ctrl+Alt+4) into both capture menus and removed the separate footer icon. Existing screenshot selection and instant-capture behavior remain intact.
- Saved links now show the domain, full URL, saved time and an open-in-browser indicator. Removed duplicate link badges; retained export badges and screenshot presentation. No remote preview requests.
- Updated design.md. TypeScript checks and all 30 frontend tests passed, including three saved-link rendering tests. Impeccable detector returned no findings; static rendering checked list/grid, including a long URL, at 540px. Native interaction and expanded browser compatibility remain unverified. No builds, branches, pushes or deployment.


## Initial video recording

- Added Record video setup for screen, native window and region, with screen-only/silent defaults. System audio, selected microphone and selected webcam are independent opt-ins. Added 30/60 fps, bitrate quality presets, resolution limits without upscaling, cursor and countdown controls, elapsed time, Pause/Resume and Stop. Capture controls are disabled while recording.
- Added a short-lived Windows x64 .NET Framework 4.8 helper using pinned ScreenRecorderLib 7.0.1 (MIT) for WGC/D3D11, WASAPI and Media Foundation recording. Rust owns session exclusion, bounded metadata/control pipes, UTF-8 configuration/status, startup/stop watchdogs, helper cleanup and private recording paths. Frames stay native. The shelf is capture-excluded while recording.
- Integrated finalized MP4s with All/Captures, folders, Windows-player playback, unique automatic export, streamed atomic Save as and confirmed deletion. Image navigation excludes videos. Interrupted/unindexed files are retained and reported; no automatic destructive recovery. Exit/updater guards protect active recordings.
- Added a locked NuGet manifest, helper build script, recorder license, app-local VC runtime packaging and release workflow SDK setup. Updated design.md and README.md, including manual build and hardware acceptance instructions. Preserved the concurrent link-dropdown/presentation and splash changes.
- Verification: TypeScript typecheck and 32 frontend tests passed; 21 native tests passed with one unrelated live-browser test ignored. C# API typechecking against the pinned dependency passed without emitting a binary. Release-version consistency and git diff whitespace checks passed. MSBuild property evaluation confirmed the expected helper output path; NuGet restore succeeded.
- Not verified: actual helper/app/installer builds, recording on real hardware, selected hardware encoder, frame pacing, CPU/memory, HDR, audio synchronization, device loss or installer runtime loading. Windows is asked for hardware encoding but may select software. Webcam composition currently uses a square inset; circular masking and mirroring are not implemented. Recovery is manual. No app/helper/installer build, branch, push or deployment was run.


## Compact capture menu

- Moved Video into both capture menus and removed its separate footer button. Shortened the labels to Region, Window, Screen, Video, Link and Instant; preserved existing shortcuts and actions.
- Fixed mixed checked/action menu alignment in the shared menu: every row reserves a check column when the menu contains checked items, with shortcut hints still right-aligned.
- Updated design.md. TypeScript typecheck, five nearby library/recording tests and focused menu-structure assertions passed. No build or native visual check was run.

## Video label and shortcut

- Corrected the capture menu label to Video and added its shortcut hint in both menus. Added configurable Ctrl+Alt+5 to Settings and the native global shortcut handler; it opens setup or existing recording controls without starting devices.
- Migrated four- and five-action settings without replacing custom bindings, choosing a free fallback when needed.
- Verified TypeScript and 13 focused tests (8 frontend, 5 native), including legacy settings, custom binding collisions and migration idempotence. No app build; real Windows shortcut input remains unverified.
