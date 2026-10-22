# Tasks — 22 October

- Fixed release-note Markdown rendering with themed, keyboard-scrollable formatting; blocked raw HTML, remote images and unsafe links. TypeScript and seven focused update tests passed.
- Added the capture-excluded recording island: Video and Ctrl+Alt+5 open it at the top of the screen with saved preferences, and it handles Record, Pause/Resume, Stop, device toggles and moving between displays by dragging or arrow keys. TypeScript, 5 recording UI tests and 4 Rust recording tests passed.
- Recording island now slides in from above the screen and back up on every close; fixed the Record button's dot turning red and disappearing into the button. TypeScript, recording tests and cargo check passed.
- Recording island placement now aligns the window's content area, not its outer frame, with the top screen edge, removing the gap above it. Cargo check passed.
