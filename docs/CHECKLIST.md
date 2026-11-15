# Development checklist

Legend: [x] implemented **and tested** as stated · [~] implemented, test coverage limited (see note) · [ ] not started.
"Tested" below means automated tests in this repo; anything that needs a real Windows desktop is listed separately.

## Milestone 1 — Core Dynamic Island UI
- [x] Project scaffold: Tauri 2 (Rust) + React/TypeScript, two windows (island, settings) — `cargo check`, `npm run build`
- [x] Settings schema, validation, merge-patch updates, atomic persistence, corrupt-file recovery — 21 Rust tests
- [x] Window placement maths: top-centre, DPI scale, negative-coordinate monitors, clamping — 7 Rust tests
- [x] Island state machine: idle / media / lyrics / assistant compact views, expanded tabbed view — browser tests
- [x] Spring animation of width / height / corner radii, 3 intensities + Off + OS reduce-motion — unit + browser tests
- [x] Hover-to-expand with delays, click-to-expand, pin, Esc, global-shortcut toggle event, stay-open-while-typing — browser tests
- [x] Music UI: artwork, title, artist, progress/seek, prev/play-pause/next, capability-aware disabling — browser tests against a simulated session
- [x] Lyrics engine: LRC + enhanced-LRC parsing, binary-search sync, position extrapolation, karaoke fill — 27 unit tests
- [x] Lyrics UI: smooth auto-scroll, depth fade/blur, click-to-seek, compact line swap animation, per-track .lrc files — browser tests
- [x] Assistant face with idle / listening / thinking / speaking / error states, text box, honest "no provider" error — browser tests
- [x] Settings window: Appearance, Island, Music & lyrics, Assistant, About; reset per section — browser tests
- [x] Tray menu, global shortcut registration with graceful failure — hotkey parse tests; runtime start-up on Linux/Xvfb
- [x] Production bundle excludes the test backend — verified by grep of `dist/`
- [ ] **Needs a Windows run (cannot be done in the Linux build environment):** transparency/shadow on WebView2, real window sizing, tray, global shortcut firing, multi-monitor, DPI, NSIS installer, uninstall
- [ ] True desktop backdrop blur (acrylic/Mica) — M1 glass is an opacity gradient only

## Milestones not started
- [ ] M2 Windows media sessions (SMTC) — fills `media_get_state` / `media_transport` and the `media-state` event
- [ ] M3 Lyrics providers (LRCLIB) — implements `LyricsProvider`
- [ ] M4 Remaining settings (startup, notifications, privacy, wallpaper accent)
- [ ] M5 AI providers (Ollama first) — fills `assistant_send`
- [ ] M6 Voice in/out
- [ ] M7 System actions + permission layer + emergency stop
- [ ] M8 Utilities · M9 Plugins · M10 Taskbar modes · M11 Installer polish, signing, performance pass
