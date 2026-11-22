# Desktop Controls and Dynamic Island customisation: QA summary (stage 5)

Stages 1-4 are committed (`071425f`, `2206461`, `b97988d` + `79f6c84`, `c06715c`) with Windows CI green. The CI job runs npm ci, type-check, vitest, `cargo test --workspace` and builds the NSIS installer (`vex-windows-installer` artifact, `tauri build`, unchanged configuration).

## What was verified here (Linux sandbox, simulated backend)
- Rust: `cargo test --workspace` (310 core tests: placement/clamping/strip, settings defaults and clamps, volume rules) and `cargo clippy --workspace --all-targets` with no warnings.
- TypeScript: type-check, vitest (drag maths, auto-hide rules, volume helpers, existing suites).
- Playwright: main UI 124, screenshot 26, redesign 10, launcher 52, hub 37, position 20, auto-hide 18 (including volume-panel and off-centre integration), volume 15, island settings 8, lyrics layout 157.
- The production build contains no test harness.

## What has not been verified (Windows code compiled by CI, never run)
`display_windows.rs` (display/DPI/resume observer), the Auto-Hide window strip and reveal in `lib.rs`, `volume.rs` / `volume_windows.rs` (Core Audio), the tray items. Everything in `docs/CHECKLIST.md` under "Needs your laptop" for positioning, Auto-Hide, volume and the Island settings is still open.

## Known risks and what to do if they show
| Risk | Symptom | Response |
|---|---|---|
| The 4 px strip does not receive the pointer on a transparent WebView2 window | Hidden island cannot be revealed from the top edge (shortcut and tray Show VEX still work) | Add cursor polling only while hidden (about 80 ms), as agreed. Not implemented. |
| Reveal steals focus | Typing in another app is interrupted when the island reappears | Window is created with focus disabled; if it still happens, show without activation (`SWP_NOACTIVATE`). |
| Core Audio callbacks fail on some drivers | Panel stuck on "Volume control is unavailable" | The message and logs name the failing step; fix per device. |

## Scope kept
No notification-driven reveal, no voice or wake-word work, no File Tray, no MSIX, Ollama settings, AI provider logic and credential storage untouched, no new crates (the `windows-core` entry is the crate `windows` already builds on).
