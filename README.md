# Islet — a free Dynamic Island for Windows

A floating pill at the top of your screen that expands into a music player with synced lyrics and an AI assistant.
Built with **Tauri 2 (Rust) + React/TypeScript** (WebView2). MIT licensed. See `NOTICE.md` for provenance — this is an
independent project, not affiliated with Notchify, Fenn or any other app.

> **Status: Milestone 2.** The island reads what is playing through Windows media sessions (Spotify, browsers, any
> player that shows in the Windows volume flyout), shows title/artist/artwork/progress and controls playback.
> The Windows build and the API integration are verified by CI; behaviour with a real player is still to be confirmed
> on a PC (see `docs/CHECKLIST.md`). Automatic lyric lookup arrives in Milestone 3.

## Run it on Windows 10/11 (x64)

1. Install [Rust](https://rustup.rs) (MSVC toolchain; the installer will offer the Visual Studio C++ Build Tools),
   [Node.js 20+](https://nodejs.org), and make sure the WebView2 runtime is present (it ships with Windows 11 and current Windows 10).
2. In this folder:
   ```powershell
   npm install
   npm run tauri dev        # run with live reload
   npm run tauri build      # produce the installer
   ```
   The installer lands in `target\release\bundle\nsis\Islet_0.1.0_x64-setup.exe` (per-user install, no admin rights; uninstall from Windows Settings).

The island appears at the top-centre of your primary display. **Hover** it (or click it, or press **Ctrl+Alt+I**) to expand.
The tray icon has *Settings…* and *Quit*.

## Try the interface without Windows
`npm install && npm run dev:browser` opens the real UI in a browser with a clearly-labelled **simulated** player (drive it from the
console via `window.__harness`). This harness is for development/testing only and is not in the shipped app.

## Using lyrics in Milestone 1
Automatic lookup arrives in M3. For now, open the island → **Lyrics** tab → **Load .lrc file…** for the current song. Plain and
enhanced (per-word) LRC are supported; the file is remembered for that track on this PC.

## Tests
```
npm test              # 27 logic tests (lyrics parsing/sync, spring, layout, accent)
cargo test --workspace  # settings, persistence, placement, media engine, hotkey parsing
npm run dev &  npm run test:ui   # 45 real-browser UI checks (needs python3 + playwright)
```
A GitHub Actions workflow (`.github/workflows/windows.yml`) runs everything and builds the installer on a real Windows runner.

## Layout
```
crates/islet-core/   pure Rust: settings schema, store, placement maths (unit-tested)
src-tauri/           Tauri shell: windows, tray, hotkey, commands
src/island/          island UI, spring animation, layout/state priority
src/lyrics/          LRC parser, sync maths, provider interface, lyric views
src/assistant/       assistant face + panel
src/settings/        settings window
src/backend/         Backend interface, Tauri implementation, dev/test harness
docs/                ARCHITECTURE.md, CHECKLIST.md
```
