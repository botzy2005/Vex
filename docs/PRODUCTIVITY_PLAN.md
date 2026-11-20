# VEX Productivity Suite: technical plan (stage 1)

Written after inspecting the repo at `9858973`. Nothing here is implemented yet; each section is
built, tested and committed as its own stage (see the order in the brief).

## What exists and will be reused

| Need | Existing piece | Reuse |
| --- | --- | --- |
| Windows, geometry | `island` window from `tauri.conf.json`; secondary windows built with `WebviewWindowBuilder` (`settings`, `editor`, `overlay-*`); multi-page Vite build | New `launcher.html` page and a pre-built, hidden `launcher` window. Windows deadlocks if a window is built inside a sync command, so it is built once in `setup` and then shown/hidden. |
| Global shortcuts | `register_hotkeys()` in `lib.rs` (`unregister_all`, per-key warnings, never fatal) | Add a third key (launcher) to the same function and the same change-detection in `update_settings`. Duplicates between keys are reported as warnings. |
| Settings | `vex-core::settings` (serde, `sanitize`, merge patch), `defaults.json` kept equal to `Settings::default()` by a cargo test | New `launcher` section, same pattern. |
| Persistence | `SettingsStore` (atomic write, corrupt-file recovery), `ChatStore`, clipboard/screenshot stores in `app_config_dir` | A small `JsonStore` helper in vex-core for launcher state (pins, recents, history) and later notes, events, widget layout. |
| AI | `AiService::prepare/execute`, key from Windows Credential Manager, provider/model from `settings.ai` | Launcher AI commands call the same service. No provider code is copied. Note: `prepare` stores a conversation; the launcher will use a one-shot request path that adds no history unless the user opens the result in the assistant. |
| Backend interface | `Backend` trait in `src/backend` (Tauri + test stand-in) | New launcher methods in both; the test stand-in serves fixture apps and files so the UI can be tested in a browser. |
| Notifications / clipboard | Windows notification listener and clipboard store | Untouched. Timer completion alerts reuse the existing notification and sound code paths later. |

## Shortcut decision

Default launcher shortcut: **Alt+Space**. Existing VEX keys: Ctrl+Alt+I (island), Ctrl+Alt+S
(screenshot). No clash among them. Alt+Space opens the window system menu in Windows, but a
registered global hotkey takes precedence (PowerToys Run uses it too). If another app already owns
it, registration fails, the user sees the warning in Settings and can record a different key. This
can only be confirmed on a real PC.

## Stage 2-7: Quick Launcher

**Window.** `launcher` window: transparent, undecorated, always-on-top, hidden, no taskbar button,
centred horizontally in the upper third of the monitor under the cursor. The shortcut shows it and
focuses it; Escape, losing focus or launching something hides it. Entrance/exit use opacity and
transform only. The page is mounted once and kept alive, so opening is a `show()` plus a state
reset, not a page load.

**Core (vex-core, platform-neutral, unit-tested).**
- `launcher::rank`: fuzzy/prefix/word-start scoring over a name list, with recency and pin boosts.
- `launcher::calc`: hand-written tokenizer and recursive-descent parser (`+ - * / % ^`, parentheses,
  unary minus, decimals, a few constants/functions). No `eval`, no code execution, depth and length
  limits.
- `launcher::commands`: a static registry (name, aliases, description, argument validator, action).
  Actions are an enum (`Calc`, `Time`, `Date`, `OpenFolder(Known)`, `OpenSettings`, `WebSearch`,
  `Timer`, `Note`, `Ai`), never a shell string.
- `launcher::store`: pins, recents, search history, with caps and clearing.
- `launcher::files`: an in-memory file index and its filters (see below).

**Application discovery (Windows shell).**
- Traditional apps: scan the user and common Start Menu `Programs` folders for `.lnk`/`.url`
  entries (names, paths), de-duplicated. Launch with `explorer.exe <path>` through `Command::arg`
  (no shell).
- Microsoft Store apps: enumerate once with a fixed, input-free PowerShell `Get-StartApps` call (hidden
  window) and launch with `explorer.exe shell:AppsFolder\<AUMID>`. The AUMID is checked against a strict
  character allowlist first.
- Result cached in memory and on disk, loaded at startup, refreshed in the background when stale
  (older than a day or when the launcher opens and the cache is older than ~10 minutes), never on a
  keystroke. Entries whose target no longer exists are dropped at launch time with a clear message
  and removed from the cache; pins to missing apps are shown as unavailable and can be removed.
- Icons: extracted lazily and cached as small PNGs for `.lnk` entries; a coloured initial tile is
  the fallback for Store apps, missing icons and failures. Icon extraction is the part most likely
  to need a second pass on real hardware.

**File and folder search.** I looked at the Windows Search index (`SystemIndex` via OLE DB/COM). It
is the best answer to "fast, no scanning", but it needs a COM/OLE DB layer that cannot be compiled or
tested locally, depends on whether the user has indexing enabled, and is a lot of unverified code.
Chosen first version: a bounded, cached name index over a small set of user-chosen scope folders
(Desktop, Documents, Downloads, Pictures, Videos, Music by default). It is built on a background
thread with caps on depth and entry count, skips hidden/system/reparse points, tolerates permission
errors per directory, and is queried from memory. Filtering by kind (document, image, video,
folder) is by extension. Rebuilt on demand and when stale. The Windows Search index can be added
behind the same `FileSource` trait later. This is a deliberate limit and will be documented.

**Web search and AI.**
- Web search runs only when the user picks the "Search the web" row or uses `search <terms>`. Only the
  typed query is placed in the URL (percent-encoded), and the URL is opened with the default browser
  through `explorer.exe`/the shell opener, https only.
- AI commands (`ask`, `explain`, `summarize`, `rewrite`, `translate`) run only on Enter and need text typed
  into the launcher. Nothing reads the clipboard or screen on its own; a "use clipboard" action
  would need an explicit user button and is out of the first version. The result row shows the
  provider and model in use. If AI is not configured the row says exactly what is missing.

**Settings.** New "Launcher" page: shortcut recorder, category toggles (apps, files, commands, web,
AI), max results (3-12), recents on/off, pinned list with remove, clear history, file scope folders.

## Stages 8-16 (not started)

Widgets Hub framework (`hub` window plus a widget registry, layout persisted in settings, lazy
loaded widgets, grid with drag reorder and 3 sizes), then Pomodoro, Notes, Calendar, World Clock,
Countdown, System Monitor, island integration, final regression pass. Timers keep **absolute
timestamps** (not tick counters) in a small Rust-owned store so they stay correct when the window
is closed or the PC sleeps; the UI only renders `now`. Pomodoro and Countdown share that timer
module. System metrics use a few Win32 calls (`GetSystemTimes`, `GlobalMemoryStatusEx`,
`GetSystemPowerStatus`, `GetIfTable2`/IP helper), polled only while the widget is visible.

## Testing approach and honest limits

- vex-core logic (ranking, calculator, command parsing, stores, file filters, URL building) is unit
  tested on Linux.
- UI (keyboard navigation, selection, empty/loading states, rapid open/close) is tested with
  Playwright against the test backend.
- Windows-only code (Start Menu scan, `Get-StartApps`, launching, icons, the window, the shortcut)
  compiles only in Windows CI and has **not** been run on a real PC. Each stage report will say so.
