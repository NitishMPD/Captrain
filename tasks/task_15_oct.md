# Tasks - 15 October 2026

## Browser URL saving, appearance and Windows startup

- Added Ctrl+Alt+4 (editable in Settings) and a shelf link button to save a page from Chrome, Edge or Brave without an extension, keystroke injection or clipboard replacement. Windows UI Automation reads visible document metadata, with an address-bar fallback; unsupported or ambiguous pages fail visibly.
- Saved HTTP/HTTPS links share library search, folders, copy, browser opening, .url export, automatic disk copies and confirmed deletion. Screenshot-only filters keep links out of image-viewer navigation. Legacy screenshot metadata and custom shortcut bindings are preserved.
- Added persisted System/Light/Dark controls using the existing palettes across the shelf and viewer. Added an opt-in Start with Windows switch backed by the current user's quoted executable Run entry; enable it from the installed copy.
- Updated design.md. No new dependencies; enabled COM and Registry features on the existing Windows crate.
- Verification: TypeScript typecheck passed; 27 frontend tests passed; 17 native tests passed. The opt-in native accessibility test also passed against a live Chrome window without focusing its address bar. Browser UI checks with mocked native commands covered link open/save actions, light/dark changes, startup state and no horizontal overflow at the native 540x740 size. Impeccable detector returned no findings.
- Not verified: Edge/Brave accessibility behavior, real global shortcut input, native link-opening/clipboard/Save dialogs, actual sign-in startup or installer behavior. No app/installer build, push or deployment. The existing installed app must be rebuilt and installed to receive these changes.
