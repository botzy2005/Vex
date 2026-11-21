# VEX Desktop Controls: architecture proposal (stage 0)

Written after auditing the repository at `05f89cc`. **No application code has been changed.** Three
capabilities: island positioning (stage 1), Auto-Hide / Always Visible (stage 2), Windows volume (stage 3),
then a Settings section (stage 4) and QA (stage 5). Each stage is committed separately and waits for
testing on the real laptop before the next begins.

## 1. Audit: what exists today

| Question | Finding |
| --- | --- |
| 1. How is the island window created and positioned? | One window, `island`, declared in `tauri.conf.json` (260x60 to start, `decorations:false`, `transparent:true`, `alwaysOnTop:true`, `skipTaskbar:true`, `shadow:false`, `focus:false`, created hidden then `show()`n at the end of `setup`). `apply_island_geometry()` in `src-tauri/src/lib.rs` sets size and position from `Settings.island` and the size the page last asked for. |
| 2. Dimensions and coordinates | The page computes the island size in `src/island/layout.ts`; the OS window is that size plus `WINDOW_PAD` (14 px sides and bottom). `useWindowSize` calls `set_island_size` (grow at once, shrink after 650 ms). Rust turns logical size into physical pixels with the monitor's scale factor and places it with `vex_core::placement::island_rect` (pure, unit-tested, handles negative monitor coordinates and clamps to the monitor). Monitor choice: `monitor_index` (-1 = primary). |
| 3. Transparency, always-on-top, focus | Set once at window creation (see above). Nothing else touches them: no `WS_EX_*` changes, no `set_ignore_cursor_events`, no `set_focus` on the island. The page decides hover/expand with DOM `pointerenter/leave` (`useExpansion`). Because the window is only slightly bigger than the island, pointer events on the window are effectively events on the island. **Not yet known:** whether the fully transparent padding receives pointer events on a real PC (see risks). |
| 4. Global mouse position / edge detection | Nothing global exists. Tauri offers `app.cursor_position()` (used once in the launcher to pick a monitor) and the windows crate has `GetCursorPos`. No mouse hook, no raw input. |
| 5. Settings storage | `vex_core::settings` (serde, `sanitize`, merge-patch updates) persisted by `SettingsStore` to `settings.json` in the app config dir (atomic write, corrupt file moved aside). `src/shared/defaults.json` must equal `Settings::default()` (a cargo test enforces it). The relevant section is `island`: `monitorIndex`, `offsetX` (logical px from centre, clamped +-2000), `marginTop`, hover/collapse timings, `hotkey`. The Settings window already has an **Island** page with a Display picker and an offset slider. |
| 6. How Appearance Studio is applied | Presets (`src/settings/presets.ts`) only patch the `appearance` section (theme, shape, glass, radius, accent). They never touch `island.*`, so position and visibility are independent of presets. The island and the live preview read the same CSS tokens. New island controls must use `tokens.css` variables so every preset and light/dark theme works. |
| 7. Rust/Tauri architecture, Windows APIs | Cargo workspace: `vex-core` (pure logic + tests) and `src-tauri` (shell). Windows-only code lives in `*_windows.rs` behind `#[cfg(windows)]` with stubs elsewhere, so it compiles only in Windows CI. The `windows` crate 0.62 is already a dependency with COM, WinRT media/notification, GDI, DWM, `Win32_UI_WindowsAndMessaging`, `Win32_UI_Shell` (which contains `SetWindowSubclass`). Existing background threads: media, notifications, clipboard, launcher, and the Hub's timer thread (sleeps when idle). |
| 8. Audio dependencies | **None.** No Core Audio code and no audio crate. `src/screenshot/sound.ts` only plays a short web-audio click. The `windows` crate 0.62 contains the Core Audio bindings (`IMMDeviceEnumerator`, `IAudioEndpointVolume`, `IAudioEndpointVolumeCallback`, `IMMNotificationClient`, `MMDeviceEnumerator`), so **no new crate is needed**, only extra feature flags. |
| 9. Tray menu and island panels | Tray (`lib.rs`): "Expand / collapse island", "Widgets Hub", "Settings...", "Quit VEX". The island expanded view is a tab bar (Music, Lyrics, Assistant, Notifications, Clipboard, Screenshot, Widgets) plus a `.tools` group (Pin open, Settings, Quit) in `src/island/App.tsx`; panel sizes live in `layout.ts`. Compact views (idle, media, lyrics, assistant, notification, screenshot) are chosen by `resolveCompactView`. |

## 2. Proposed architecture

```
 island page (React)                         Rust shell (src-tauri)                     vex-core (pure, tested)
 ------------------                          ----------------------                     -----------------------
 drag grip / Alt+drag  --set_island_offset-> placement + persist (once, on release) --> placement::island_rect (+ clamp helpers)
 hidden/visible state  --set_island_hidden-> geometry: full size or thin edge strip --> placement::strip_rect
 volume popover        --volume_* commands-> volume thread (COM, MTA) ---------------> volume::{State, step, clamp}
         ^                                          |  IAudioEndpointVolumeCallback (events, no polling)
         '------ "volume-state" event <-------------'  IMMNotificationClient (default device changed)
                                            display/DPI/resume --WM_DISPLAYCHANGE etc.--> re-apply geometry (debounced)
```

Principles: reuse `apply_island_geometry` and `island_rect` rather than adding a second placement path;
every new setting goes through the existing settings store; Windows specifics stay in `*_windows.rs`; no
polling loops, no shell or PowerShell calls.

## 3. Files and modules to change or add

Stage 1 (positioning)
- `crates/vex-core/src/placement.rs`: helpers to turn a pointer x into an offset and clamp it to the monitor (pure, tests); raise the `offsetX` clamp so ultra-wide monitors work.
- `crates/vex-core/src/settings.rs` + `src/shared/defaults.json` + `src/shared/types.ts`: only widen the `offsetX` range (no new field).
- `src-tauri/src/lib.rs`: command `set_island_offset(offset_x, commit)`; tray item "Reset island position".
- `src/island/` (new `useIslandDrag.ts`, small edits to `App.tsx`, `island.css`): drag grip and Alt+drag; `src/backend/*` interface, Tauri binding and test stand-in.
- `src/settings/App.tsx` (Island page): position readout, Reset Position.

Stage 2 (visibility)
- `settings.rs`/`defaults.json`/`types.ts`: `island.visibility` (`always` default, `auto`), `island.autoHideDelayMs`.
- `placement.rs`: `strip_rect` (the thin edge strip) + tests.
- `lib.rs`: `set_island_hidden`, geometry honours it, re-apply on display change; new `src-tauri/src/display_windows.rs` (window subclass for `WM_DISPLAYCHANGE`, `WM_DPICHANGED`, resume) with a no-op stub elsewhere.
- `src/island/hooks.ts` (`useAutoHide`, extends `useExpansion` only through a new "hold" input), `App.tsx`.

Stage 3 (volume)
- New `crates/vex-core/src/volume.rs` (state, percent rounding, step maths, error text) with tests.
- New `src-tauri/src/volume.rs` (commands, state, events) and `volume_windows.rs` (Core Audio), stub on other platforms.
- `src-tauri/Cargo.toml`: add windows features `Win32_Media_Audio`, `Win32_Media_Audio_Endpoints` (and `Win32_Foundation` / `Win32_UI_Shell_PropertiesSystem` if the compiler asks). **No new crate.**
- `src/island/VolumePopover.tsx`, `icons.tsx`, `App.tsx`; backend interface, Tauri binding and test stand-in.
- `settings.rs`/`defaults.json`/`types.ts`: `island.volumeButton` (default on), `island.volumeStep` (default 5, range 1-20).

Stage 4 (settings): `src/settings/App.tsx` Island page grouped into Position / Visibility / Volume, `settings.css` only if needed.
Stage 5: tests, `docs/CHECKLIST.md`, final regression.

New dependencies: **none.** New module files: `volume.rs`, `volume_windows.rs`, `display_windows.rs`, `useIslandDrag.ts`, `VolumePopover.tsx`.

## 4. Windows API choices and justification

| Need | Choice | Why / rejected alternatives |
| --- | --- | --- |
| Volume read/write | **Core Audio**: `IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender, eMultimedia)` then `IAudioEndpointVolume` (`Get/SetMasterVolumeLevelScalar`, `Get/SetMute`) | The OS's own master volume; scalar matches the Windows slider. No admin rights. Rejected: simulating `VK_VOLUME_*` key presses (shows the OS flyout, cannot set an exact level), PowerShell/COM scripts (forbidden by the brief and slow), per-app session volume (not what was asked). |
| External and keyboard-key changes | `IAudioEndpointVolumeCallback` via `RegisterControlChangeNotify` | Event-driven: Windows calls us when the volume or mute changes for any reason, including the keyboard volume keys, so no polling. |
| Default device changes | `IMMNotificationClient::OnDefaultDeviceChanged` | Event-driven; we re-bind the endpoint and re-publish state. Device removed with no replacement gives an "unavailable" state, not an error loop. |
| Edge detection / reveal | **The island window itself shrinks to a thin strip at the top edge while hidden** (see section 7); pointer events on that strip reveal it | Event-driven (no timer, no polling, no hook, no extra process or window). Rejected as primary: `WH_MOUSE_LL` global hook (system-wide input latency, security software flags it, needs its own message loop), raw input sink (fires on every mouse move system-wide), `GetCursorPos` polling (works, but a wake-up every tens of ms; kept only as a fallback). |
| Display / DPI / resume changes | `SetWindowSubclass` on the island HWND for `WM_DISPLAYCHANGE`, `WM_DPICHANGED`, `WM_SETTINGCHANGE` (work area) and `WM_POWERBROADCAST` (resume), debounced ~300 ms to `apply_island_geometry` | Tauri exposes no display-change event. `Win32_UI_Shell` is already enabled. Fallback if subclassing misbehaves: re-apply geometry on window focus/visibility changes and on every reveal (cheap). |
| Dragging | JavaScript pointer events to `set_island_offset`, not `startDragging()` | The OS drag allows vertical movement and would need constant clamping back, which jitters. JS dragging sends only the x offset; Rust clamps and moves the window. |

## 5. State management and persistence

- **Persisted** (existing `settings.json`, merge patches, `sanitize`, `defaults.json` parity test): `island.offsetX` (reused: logical px from the monitor's centre, so existing users keep their position and centre stays the default), `island.visibility`, `island.autoHideDelayMs`, `island.volumeButton`, `island.volumeStep`. No separate settings system.
- **Position semantics:** centre-relative logical pixels, clamped at placement time. Drag writes the setting **once, on release** (not every move). If the display later shrinks, the placement clamps but the saved value is not rewritten, so returning to the larger display restores the position. Reset sets `offsetX` to 0.
- **Not persisted** (in memory): hidden/visible, drag in progress, current volume (read from Windows each time).
- Window geometry is derived: settings + requested size + hidden flag + monitor list, all through `apply_island_geometry`.

## 6. Multi-monitor considerations

- The island lives on one chosen monitor (`monitorIndex`, -1 = primary). Dragging moves it only along that monitor's top edge. Moving it to another monitor stays a Settings choice (the brief asks for no placement outside the selected monitor).
- All maths is in physical pixels with each monitor's own scale factor, including monitors at negative coordinates (existing tests) and mixed DPI. New tests add left/right clamps at 100/150/200 % and a monitor above or left of the primary.
- `monitorIndex` is an index into the OS list, which can reorder when monitors are plugged in or out. Fallback: out-of-range index uses the primary (already implemented). I will not change this model in these stages; it is listed as a known limit.
- Auto-Hide: the strip belongs to the island's monitor only. With another monitor stacked above it, the cursor passes through the strip while moving between monitors and may briefly reveal the island; the hide delay absorbs that.

## 7. Auto-Hide behaviour and edge detection

**Always Visible** is the default and changes nothing about today's behaviour.

**Auto-Hide state machine** (in the page, using the existing pointer events):
`visible -> (pointer outside island, not held, delay elapsed) -> hiding -> hidden`, `hidden -> (pointer enters the edge strip) -> revealing -> visible`.
- **Hidden state:** the page slides the island out of view (opacity/transform only), then asks Rust `set_island_hidden(true)`. Rust shrinks the window to the island's width by a few pixels tall (`strip_rect`), still at the top of the monitor, still always-on-top. The strip contains a near-invisible hit element (alpha about 1/255) so Windows delivers pointer events to it.
- **Reveal:** `pointerenter` on the strip calls `set_island_hidden(false)` (Rust restores the full size without activating the window) and the island slides in. It does not take keyboard focus (the window is never activated; no `set_focus`).
- **Held visible while:** expanded and pinned; pointer inside; a text field focused (the assistant input already sets a lock); a toast showing (screenshot confirmation); the volume popover open; a drag in progress; Settings "preview" off. Only when none apply does the hide delay (default 800 ms, adjustable) start.
- **No flicker:** hysteresis. The reveal region (strip) is the only way to reveal; the hide timer only starts after the pointer has left the full island; any re-entry cancels it; and a minimum visible time (about 400 ms) after a reveal prevents reveal-hide-reveal loops at the edge.
- **Idle cost:** zero timers while visible and idle; one short timer only during the hide delay; no polling at all in the primary design.
- **Events that should still be seen:** the global shortcut and the tray "Expand / collapse island" reveal the island; a screenshot confirmation reveals it for its normal few seconds, then it hides again. Notification previews and timer alerts will **not** reveal it unless you ask (kept out of scope).
- **Sleep/wake and display change:** the subclass hook re-applies geometry and re-asserts always-on-top; a hidden island stays hidden.
- **Fallback if the strip does not receive pointer events on your PC (the main risk):** an optional "compatibility edge detection" that polls the cursor every ~80 ms **only while hidden and Auto-Hide is on** (a single `GetCursorPos` per tick), stopped the moment the island reveals. Used only if hardware testing shows the strip approach fails.

## 8. Volume-control integration

- **Thread:** one dedicated thread (like the media and Hub threads) initialises COM (MTA), binds the default render endpoint, registers the volume callback and the device notification client, then blocks on a command channel. Zero CPU while idle.
- **State sent to the page** (`volume-state` event, small): `{available, percent 0-100, muted, device?, error?}`. The percent is the OS scalar rounded to 1 %.
- **Commands:** `volume_get`, `volume_set(percent)`, `volume_step(delta)` (default +-5, from `island.volumeStep`), `volume_toggle_mute`. Values are clamped in `vex-core::volume`, so a bad number can never be sent to Windows. Setting a level while muted unmutes, matching what Windows does.
- **Reflecting outside changes:** the callback pushes every change (keyboard keys, other apps, the Windows flyout) as a new event. The slider ignores echoes while the user is dragging it, so it does not fight the pointer.
- **Slider traffic:** the page throttles slider input to about 30 updates per second.
- **Device changes and errors:** default device changed means re-bind and re-publish; no device means `available:false` and a clear "No audio output device" message with disabled controls; a failed COM call keeps the previous state, shows a short message and retries on the next device notification. No error text contains device identifiers beyond the friendly name.
- **UI:** a speaker icon button in the expanded island's tools group (next to Pin and Settings). The icon changes for muted, low, medium and high. Clicking opens a compact popover inside the island (slider, percentage, minus, plus, mute) built from the shared tokens, so every Appearance Studio preset and the light theme apply. The popover counts as "held" for Auto-Hide. When `volumeButton` is off the icon is not rendered and the volume thread is not started.
- **Not in scope:** per-app volume, output device switching, showing the volume in the collapsed island.

## 9. Testing strategy

- **vex-core (Linux):** `placement` (offset to window x, clamps at 100/150/200 % and negative coordinates, narrow monitors, the strip rect), `volume` (percent rounding, step clamping at 0 and 100, mute interplay, error text), settings sanitise and the existing defaults parity test.
- **vitest:** drag maths and offset conversion, auto-hide state machine as a pure reducer (reveal, hide delay, holds, no loop within the minimum visible time), volume icon selection, slider throttling.
- **Playwright (test backend):** drag handle moves the island and the saved offset; interactive controls inside the island never start a drag; reset position; Always Visible never hides; Auto-Hide hides after the delay, reveals on the strip, stays while the assistant input is focused or the popover is open, does not flicker (count visibility changes), no idle timers (count `setTimeout`/`setInterval`); volume slider, plus/minus, mute, external change arriving as an event, unavailable device, failing call; all existing suites unchanged.
- **Windows CI:** compile, cargo tests, NSIS build. CI proves compilation only.
- **Needs your laptop (will be listed in `docs/CHECKLIST.md` per stage):** dragging feels right and stays on the top edge; position survives restart; resolution/DPI change; a second monitor; the strip really receives the pointer on a transparent window; reveal does not steal focus from the app you are typing in; no flicker; always-on-top over normal windows; volume slider/mute/keys/Windows flyout stay in sync; unplug headphones or switch output device.
- I will not claim any of the Windows behaviour is verified until you have tested it.

## 10. Risks and fallbacks

| Risk | Likelihood / impact | Mitigation / fallback |
| --- | --- | --- |
| Transparent strip may not receive pointer events (alpha-0 areas of WebView2 windows) | Medium; Auto-Hide could never reveal | Near-invisible hit element; verify on your PC first thing in stage 2; fallback cursor polling only while hidden (section 7). |
| Reveal steals focus or flashes the taskbar | Low-medium | Window is never activated or focused on reveal (resize and move only); if Windows still activates it, set `WS_EX_NOACTIVATE` on the island. |
| Always-on-top fights a full-screen game or video | Medium (cosmetic) | Auto-Hide helps (only a few pixels remain); document that exclusive full screen hides it; no new behaviour added. |
| Dragging conflicts with clicks or text selection | Medium | Dedicated grip (shown on hover) plus an explicit Alt+drag gesture; never starts on buttons, inputs or lyrics; movement threshold of a few pixels before a drag begins. |
| Window resize while dragging flickers (the shrink-after-delay logic in `useWindowSize`) | Low-medium | Drag only moves the window (no size change); sizes stay as today. |
| Expanded panel near a screen edge | Certain, cosmetic | The existing clamp shifts the window inward when it grows; the collapsed pill keeps its saved position. Covered by tests. |
| Core Audio code cannot be compiled or run locally | High for iteration time | Isolated in `volume_windows.rs`; pure logic tested on Linux; expect one or two CI fix rounds (as with the launcher); a stub keeps non-Windows builds working. |
| COM threading mistakes (callbacks on COM threads, object lifetime) | Medium | Single owner thread, callbacks only send plain data on a channel, unregister on shutdown and on device change. |
| Subclass hook interacting badly with the webview | Low | Subclass is tiny, only forwards messages, and is removed on exit; fallback to re-applying geometry on focus/visibility. |
| `monitorIndex` shifts when monitors change | Medium, existing | Out-of-range falls back to primary; documented limit; not changed here. |
| Position stored in logical px differs after a DPI change | Low | Maths done in physical pixels per monitor at placement time; clamp handles overflow. |

## 11. Proposed implementation sequence

1. **Stage 1: positioning.** Pure placement maths and tests, `set_island_offset`, drag grip + Alt+drag, Reset Position (Settings and tray), UI tests, commit, CI.
2. **Stage 2: Auto-Hide.** Settings fields, `strip_rect`, hidden-state geometry, auto-hide reducer and hook, display-change subclass, tests, commit, CI. First hardware question: does the strip receive the pointer?
3. **Stage 3: volume.** `vex-core::volume`, Core Audio thread, events, popover, tests, commit, CI.
4. **Stage 4: Settings section** for position, visibility and volume.
5. **Stage 5: QA**, regression suites, NSIS, checklist.

After each stage I stop and wait for your test result on the laptop before continuing.

## 12. Decisions I would like from you

1. **Edge detection:** is the primary design acceptable (the island shrinks to a thin, nearly invisible strip at the top edge; no hook, no polling), with cursor polling only as a fallback if the strip fails on your PC?
2. **Drag gesture:** a visible grip on hover plus Alt+drag anywhere on non-interactive island surface. Fine?
3. **Volume button:** only in the expanded island (not in the small collapsed pill). Fine?
4. **Auto-Hide reveal events:** shortcut, tray and the screenshot confirmation reveal the island; notifications and timers do not. Fine?
