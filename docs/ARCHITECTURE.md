# Architecture (Milestone 1)

```
┌────────────────────────── Tauri shell (src-tauri, Rust) ──────────────────────────┐
│ window placement · settings state · tray · global shortcut · command surface      │
│   uses ▶ crates/islet-core  (pure Rust: settings schema, store, placement maths)  │
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
  Rust validates/clamps, writes atomically to `%APPDATA%\app.islet.desktop\settings.json`, then broadcasts
  `settings-changed` to every window. `src/shared/defaults.json` is checked against Rust defaults by a test.
* **Island window** — one small transparent always-on-top window. The UI springs the island's size inside it and
  tells Rust (`set_island_size`) how big the window must be; Rust re-centres it on the chosen monitor with DPI maths.
  It grows before the animation and shrinks after it, so nothing is clipped.
* **Compact view priority** — assistant activity > current lyric line > now playing > idle. New activities
  (notification, timer, system, file transfer) slot into `resolveCompactView` in `src/island/layout.ts`.

## Interfaces waiting for later milestones
| Interface | Today | Fulfilled by |
|---|---|---|
| `media_get_state` / `media-state` event / `media_transport` | no session; transport returns "not available yet" | M2 — Windows media sessions |
| `LyricsProvider` (`src/lyrics/provider.ts`) | only the user's own `.lrc` files | M3 — LRCLIB + fallback chain |
| `assistant_send` / `assistant-state` event | sending reports "No AI provider is connected yet" | M5 (providers), M6 (voice) |
| Settings `ai.*` | stored, not used | M5 |
