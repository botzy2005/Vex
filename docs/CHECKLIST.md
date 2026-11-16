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

## Milestone 2 — Windows media sessions
- [x] Which session to show when several players are open (playing > paused > stopped, Windows' current session wins ties) — 5 Rust tests
- [x] Position maths: start offset, rate, stale update times, clamping, Windows tick conversion — 7 Rust tests
- [x] Change detection: no UI spam from normal playback; seeks, track/status/capability/artwork changes are emitted — 3 Rust tests
- [x] Artwork: fetched once per track, retried while the player is late publishing it, bounded, never reused across tracks, only real images accepted — 4 Rust tests
- [x] Polling loop: commands handled immediately, quick re-poll after a command, clean shutdown — threaded Rust test
- [x] Wire format of state and transport commands matches the TypeScript types — 2 Rust tests
- [x] Windows glue (`media_windows.rs`) compiles on Windows and the real API is reachable: on a GitHub Windows runner the session manager initialises and listing sessions succeeds (reports "nothing playing")
- [ ] **Not yet verified with a real player (needs a human at a Windows PC):** title/artist/artwork from Spotify and a browser tab, play/pause/next/previous, seeking by clicking the bar or a lyric line, progress staying in sync, switching between two open players

## Milestone 3 — automatic lyrics, letter animation, redesign
- [x] LRCLIB lookup logic in `islet-core::lyrics`: title/artist cleaning, URL building, duration-checked candidate scoring, miss cache (errors not cached) — 14 Rust tests with a fake HTTP layer
- [x] `lyrics_fetch` Tauri command with `ureq` and an identifying User-Agent; honours the `lyricsOnline` setting (off = nothing leaves the PC)
- [x] Provider chain: your own .lrc first, then LRCLIB; plain-only, instrumental, not-found and network-error states are shown honestly; auto-retry after 20 s
- [x] Per-letter animation (`LetterLine`): staggered rise-in, left-to-right sweep. Follows real word timing when the lyrics have it; for plain LRC the sweep speed is an **estimate** (~70 ms/letter, capped by the line length) — 6 unit tests
- [x] Settings: "Find lyrics automatically", "Lyrics animation" (Letters/Line)
- [x] Redesign: pure-black surface, drifting ambient glow from the artwork, larger artwork with glow, current lyric inside the player — 57 browser checks, screenshots reviewed
- [ ] **Not verified (sandbox cannot reach lrclib.net, so no live request has ever been made):** real lookups on a PC, match quality across services (Spotify / YouTube titles), letter-animation smoothness on real hardware

## Milestones not started
- [ ] M4 Remaining settings (startup, notifications, privacy, wallpaper accent)
- [ ] M5 AI providers (Ollama first) — fills `assistant_send`
- [ ] M6 Voice in/out
- [ ] M7 System actions + permission layer + emergency stop
- [ ] M8 Utilities · M9 Plugins · M10 Taskbar modes · M11 Installer polish, signing, performance pass
