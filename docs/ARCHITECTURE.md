# Architecture (Milestone 1)

```
┌────────────────────────── Tauri shell (src-tauri, Rust) ──────────────────────────┐
│ window placement · settings state · tray · global shortcut · command surface      │
│   uses ▶ crates/vex-core  (pure Rust: settings schema, store, placement maths)  │
└───────────────▲──────────────────────────────────────────────▲────────────────────┘
        invoke / events                                        │ invoke / events
┌───────────────┴────────────────┐                ┌────────────┴─────────────┐
│ island window  (index.html)    │                │ settings window          │
│ React UI, spring animation     │                │ (settings.html)          │
└────────────────────────────────┘                └──────────────────────────┘
```

* **`Backend` interface (`src/backend/backend.ts`)** is the only thing the UI knows about the native side.
  `tauri.ts` maps it onto Rust commands. `test.ts` is an in-browser stand-in used by `npm run dev:browser`
  and the automated UI tests; it is compiled out of production builds.
* **Settings** — Rust is the source of truth. The UI sends RFC 7386 merge patches (`null` resets a field),
  Rust validates/clamps, writes atomically to `%APPDATA%\com.vex.desktop\settings.json`, then broadcasts
  `settings-changed` to every window. `src/shared/defaults.json` is checked against Rust defaults by a test.
* **Island window** — one small transparent always-on-top window. The UI springs the island's size inside it and
  tells Rust (`set_island_size`) how big the window must be; Rust re-centres it on the chosen monitor with DPI maths.
  It grows before the animation and shrinks after it, so nothing is clipped.
* **Compact view priority** — assistant activity > current lyric line > now playing > idle. New activities
  (notification, timer, system, file transfer) slot into `resolveCompactView` in `src/island/layout.ts`.

## Interfaces waiting for later milestones
| Interface | Today | Fulfilled by |
|---|---|---|
| `LyricsProvider` (`src/lyrics/provider.ts`) | user `.lrc` files, then LRCLIB (native fetch in `src-tauri/src/lyrics.rs`, logic in `vex-core::lyrics`) | more providers / manual search |
| `ai_send` / `ai-event` stream, `ai_*` and `chat_*` commands | real chat: Ollama, OpenAI, Gemini, OpenRouter, Claude, OpenAI-compatible; history in `chats.json`; keys in Windows Credential Manager | M6 (voice) |
| `assistant-state` event | drives the assistant face (thinking while an answer is on its way) | M6 (voice) |

## Media (Milestone 2)
```
Windows SMTC ─▶ media_windows.rs (reads state, forwards commands)      ← only Windows-specific code
                    │ implements MediaSource
                    ▼
        vex_core::media::MediaEngine   pick session · position maths · change detection · artwork cache
                    ▲ run_loop (own thread, 500 ms active / 1.5 s idle / 120 ms after a command)
                    │
        src-tauri/src/media.rs ──▶ `media-state` event + `media_get_state` / `media_transport` commands ──▶ UI
```
The UI never polls: it receives a state only when something changed and extrapolates the playback position itself
between updates. On non-Windows platforms an `UnavailableSource` is used, so the UI shows "Nothing playing" and
commands fail with a clear message.
