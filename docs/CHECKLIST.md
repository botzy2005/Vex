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
- [x] LRCLIB lookup logic in `vex-core::lyrics`: title/artist cleaning, URL building, duration-checked candidate scoring, miss cache (errors not cached) — 14 Rust tests with a fake HTTP layer
- [x] `lyrics_fetch` Tauri command with `ureq` and an identifying User-Agent; honours the `lyricsOnline` setting (off = nothing leaves the PC)
- [x] Provider chain: your own .lrc first, then LRCLIB; plain-only, instrumental, not-found and network-error states are shown honestly; auto-retry after 20 s
- [x] Per-letter animation (`LetterLine`): staggered rise-in, left-to-right sweep. Follows real word timing when the lyrics have it; for plain LRC the sweep speed is an **estimate** (~70 ms/letter, capped by the line length) — 6 unit tests
- [x] Settings: "Find lyrics automatically", "Lyrics animation" (Letters/Line)
- [x] Redesign: pure-black surface, drifting ambient glow from the artwork, larger artwork with glow, current lyric inside the player — 57 browser checks, screenshots reviewed
- [ ] **Not verified (sandbox cannot reach lrclib.net, so no live request has ever been made):** real lookups on a PC, match quality across services (Spotify / YouTube titles), letter-animation smoothness on real hardware

## Milestone 5 — AI chat
- [x] Provider layer in `vex-core::ai`: request builders and streaming parsers for Ollama (NDJSON), OpenAI / OpenRouter / OpenAI-compatible (SSE), Gemini (SSE), Claude (SSE); model-list parsing; error classification that never echoes keys — 50+ Rust unit tests
- [x] `AiService` (shell): history, one request at a time, cancel, key handling. Tested end to end with the real `ureq` transport against local HTTP servers that speak each provider's wire format: streaming arrives incrementally, follow-ups carry context, keys go only in headers, Stop keeps the partial reply, Ollama down / model missing / no model all give actionable errors, nothing is stored for refused messages — 19 shell tests
- [x] Real Anthropic 401 response (captured from api.anthropic.com with a fake key) is classified as a bad key without echoing it
- [x] Credentials: Windows Credential Manager via `keyring` (Windows build only; in-memory on Linux dev builds). Keys are never returned to the UI, logged or written to settings
- [x] Chat UI: streaming, typing indicator, Stop, Markdown (own safe parser, no HTML injection), new chat, history list with timestamps, delete, clear all, persistence across restart
- [x] Settings → Assistant: provider, per-provider model, API key save/clear, Ollama and custom endpoints, Test connection, installed-model list
- [x] App context only when asked (intent detection + one-shot buttons + a setting to turn it off), shown on the message — unit + browser tests
- [x] Browser tests (79) with a scripted model double; unit tests (45)
- [ ] **Not verified (needs your PC):** a real Ollama conversation and model list; real OpenAI / Gemini / OpenRouter / Claude replies with a valid key (their hosts are not reachable from the build sandbox, so those parsers are checked against the documented wire formats only); Windows Credential Manager round trip beyond the CI check; chat UI look and feel inside the real transparent window
- [ ] Not done: clickable links in replies (shown as text with the address), proxy support, voice (Milestone 6)

## AI performance work (local Ollama)
- [x] **Likely cause found in the code:** the old Ollama request was only `{model, stream, messages}`: no `think` setting (reasoning models such as Qwen think by default), no `keep_alive`, no context size; and the parser read only `message.content`, so hidden reasoning produced no visible sign of life for the whole thinking time. **Not yet confirmed by a measurement on the target laptop.**
- [x] Request now sends `think` (default off), `keep_alive` (default 30 min) and `num_ctx` (default 4096); GPU layers are left to Ollama (not forced). All three are settings.
- [x] History and shared music context are trimmed to the context window (a prompt longer than `num_ctx` is cut by Ollama and slow to read).
- [x] Hidden reasoning is reported as progress ("Thinking · 12s"), never shown or stored; a silent wait shows elapsed time; Stop is immediate even while Ollama is silent.
- [x] Streamed text is applied once per frame instead of once per chunk; repeated Enter sends exactly one request.
- [x] Per-request measurements (numbers only, never text or keys) in Settings → Developer: first word, thinking time, total, load, tokens, tokens/s, payload size, messages, prompt size, settings used.
- [x] Developer speed test: same prompt straight to Ollama and through VEX, per installed model, optional thinking-on pass, capped at 128 tokens, never downloads.
- [x] Measured (stand-in server, automated): VEX's pipeline adds ~0 ms to the first word and ~1 ms in total versus a bare request (`cargo test -p vex adds_negligible_latency -- --nocapture`).
- [ ] **Needs the target laptop:** real before/after times, tokens/s and the 2B vs 4B comparison for qwen3.5 on the RTX 3050 (4 GB). Use Settings → Developer → Ollama speed test.

## Milestones not started
- [ ] M4 Remaining settings (startup, notifications, privacy, wallpaper accent)
- [ ] M6 Voice in/out
- [ ] M7 System actions + permission layer + emergency stop
- [ ] M8 Utilities · M9 Plugins · M10 Taskbar modes · M11 Installer polish, signing, performance pass

## Context detection and default prompt fix

Root causes (found by probing, before changing anything):
- "What song am I currently listening to?" matched none of the old detector's patterns, so no music context was attached and the model answered truthfully that it could not see it. The Windows media integration itself was fine; the old UI test only used "What song is playing?".
- The default system prompt said "Keep replies short." and said nothing about substance or about the shared-context block.

Fixed and covered by automated tests (vitest, cargo, Playwright with the test backend):
- Broader song and lyrics detection, with unrelated questions staying private. Setting off means only the Share chips attach context. Honest "nothing playing" / "no lyrics" text.
- New default prompt, a one-time upgrade of the untouched old default (custom prompts are kept), and a "Reset to default" button.

Needs the user's PC (not verifiable in the sandbox):
- [ ] Answer quality of qwen3.5:2b / 4b with the new prompt
- [ ] Real Windows media session: ask "What song am I currently listening to?" with music playing


## Notifications module

Automated (Linux sandbox + Windows CI): parsing, filtering, de-duplication, grouping (TS), history limits, dismissal,
clearing, privacy rules (previews off keeps no text, disabling forgets everything, existing Action Center items are not read,
nothing in memory is written to disk), engine loop with a fake source, settings sanitising, UI behaviour with a simulated source.

Windows limitation found while building it: `UserNotificationListener` only works for apps with **package identity**. The NSIS
installer build has none, so Windows may report "Allowed" and then fail reads with "Element not found". VEX probes this and
shows an honest "unavailable" message instead of an empty list. Getting identity means shipping a (sparse) MSIX package with a
signing certificate the user's PC trusts; that is a packaging decision, not done here.

Needs the user's PC:
- [ ] Open Settings -> Notifications, switch it on, accept the Windows prompt. Read the status line: it says either that
      permission was given, or why notifications are unavailable (see above).
- [ ] If allowed: trigger a notification from any app; confirm the island preview, panel, grouping, remove, open-app.
- [ ] CI step "Windows media API smoke test" now also prints what the real listener returned on a runner.

## Clipboard history

Built and tested in the sandbox (Rust core tests with a fake clipboard, vitest, Playwright with a simulated source):
- Text and image history, newest first, pinned first; an immediate repeat is stored once; search; pin; delete; Clear (keeps pinned); size limit that never trims pinned entries.
- Off by default. Nothing copied before enabling, resuming, or during a pause is ever read. Disabling forgets the history. Persistence is off by default; turning it off deletes saved files. Saved files are not encrypted.
- Password-manager copies (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory = 0`) and text that looks like a key or card number are skipped, with a notice. The shape filter is a heuristic and can miss things.
- The tab shows a red dot while recording; the panel and Settings always state whether it is recording, paused or off.
- Clipboard content is never sent to the assistant (covered by a UI test) and events carry only the status, not content.

Needs your PC (the Windows code is only compiled by CI here, never run):
- [ ] Copy text in another app: it appears in the Clipboard tab within about half a second
- [ ] Copy an image (Snipping Tool, browser "Copy image"): thumbnail appears; click it and paste into Paint/Word: quality is unchanged
- [ ] Copy a password from a password manager: it is not recorded
- [ ] Pause, copy something, resume: the copy made while paused does not appear

## Screenshot

Built and tested in the sandbox (Rust core tests, vitest, Playwright against a simulated backend):
- Geometry: mapping a drag in CSS pixels to physical pixels per monitor (mixed DPI), window-rect clipping to the desktop, exact crops, size limits (40 MP), PNG and JPEG encoding (JPEG flattens transparency onto white), file naming without overwriting, history limits (Off 1 / Session 10 / Keep 30, Keep stored on disk, leaving Keep deletes the files).
- Editor (pixel-checked in a real browser canvas): crop, rotate both ways, resize (locked and free, bad sizes rejected), rectangle, ellipse, line, arrow, highlight, text, blur and pixelate, undo/redo, Copy and Save send the edited PNG.
- Region overlay: live size label in physical pixels (checked at 2x), exact rectangle reported, Escape/click do not capture.
- Island: confirmation with thumbnail and Edit / Copy / Save, auto-dismiss, error message, Screenshot tab with three capture buttons, list, quick preview, delete, clear. Settings page: format, folder, shortcut (can be turned off), shortcut mode, sound, history.
- Screenshots never leave the PC unless you save or copy them yourself. Nothing is captured in the background.

Needs your PC (the Windows capture code is only compiled by CI here, never run):
- [ ] Select area: drag on the screen; the saved picture is exactly the dragged region, at full resolution
- [ ] Window: the picture is the window you were using, without a shadow border gap, not VEX
- [ ] Full screen: captures the display the pointer is on, at its real resolution
- [ ] Two monitors with different scaling (for example 100% and 150%): each region and the full screen are sharp and correctly sized
- [ ] The VEX island does not appear in any screenshot
- [ ] Copy, then paste into Paint/Word/chat: image is unchanged; Save writes PNG or JPEG to the chosen folder; Save as opens the native dialog
- [ ] The shortcut works from other apps; changing it in Settings takes effect without restarting; turning it off frees the keys
- [ ] The capture sound plays when enabled
- [ ] Keep history: restart VEX and the screenshots are still listed; switching to another option deletes them

Known limits: a region cannot span two monitors; "full screen" is one display, not all of them; the exclusion of the island from the capture relies on a Windows feature that is untested on real hardware; kept history files are not encrypted.

## Widgets Hub (framework, stage 8)

Built and tested in the sandbox (Rust core tests, vitest, Playwright against a simulated backend with probe widgets):
- Layout: add, remove, resize (only allowed sizes), pin (max 4), reorder by dragging the handle, by Alt+arrow keys and by buttons; saved on every change, restored after a reload; an emptied dashboard stays empty; Reset layout asks first; a layout changed in another window appears at once.
- Lifecycle: a widget's code loads only when its card first shows; widgets are told when the Hub is hidden; a widget that throws shows an error on its own card (Reload / Remove) while the others keep working; failed saves are reported.
- Widget data: one JSON document per widget, debounced saves (one write for rapid typing), flushed when the widget goes away, 512 KB limit, unknown or path-like widget ids refused (Rust tests).
- Shared timers (Rust engine, fake-clock tests): absolute end times, pause/resume, reset, restart while running, finished-while-closed reported once, sleep/wake jump, limits, damaged-file repair.
- Island: a Widgets tab with pinned widgets, widget status lines and running timers; no interval runs while no timer is running.
- Nothing from the Hub is sent anywhere or given to the AI.

Needs your PC (the Windows code is only compiled by CI here, never run):
- [ ] Tray menu > Widgets Hub opens the window; the island's Widgets tab > Open Hub opens the same window; closing it and reopening restores your layout
- [ ] Customise: dragging a card by its handle works with the mouse on a real window (the drag does not rely on browser drag-and-drop)
- [ ] Pinned widgets appear in the island's Widgets tab; clicking one opens (or focuses) the Hub on that widget
- [ ] Closing the Hub leaves no extra process or noticeable CPU use (the only resident part is the timer thread, which sleeps with no running timer)
- [ ] The Hub follows the light/dark theme and accent colour from Settings

Known limits: there is one real widget ("Getting started"); the six productivity widgets come in the next stages. Timers follow the system clock if you change it while one is running. There is no desktop notification or sound on timer completion yet (decided with the Pomodoro stage). The compact island does not show timers yet (stage 15).

## Island positioning (desktop controls, stage 1)

Built and tested in the sandbox (Rust core tests, vitest, Playwright against a simulated backend):
- Placement maths: the furthest the island may sit from the centre, and clamping, for several monitor widths and scales (100/125/150/175/200 %), monitors at negative coordinates and an ultra-wide monitor; a clamped offset never puts the window past either edge, and clamping twice changes nothing.
- Dragging: from the grip (shown at the island's bottom edge while hovered) or Alt+drag on the island surface. It starts only after a few pixels of sideways movement, so clicks (even Alt+clicks) still press buttons; it never starts inside a text field; the click that ends a drag is swallowed; Escape puts the island back; the island stays open during a drag; pointer movement is batched per frame; the position is saved once, on release; it persists across reloads and the next drag continues from it.
- The grip overlaps no button, field or slider on any tab (checked on all seven tabs).
- Settings > Island: the position readout ("Centre", "180px left"), and Reset position (also in the tray menu: "Reset island position").
- Resolution / DPI / monitor / work-area / resume changes re-place the island (a small observer on the island window, debounced).

Needs your laptop (the Windows code is only compiled by CI here, never run):
- [ ] Hover the island: the grip appears at its bottom edge. Drag it left and right: the island follows the pointer smoothly, stays on the top edge, and stops at the screen edges
- [ ] Hold Alt and drag the island's surface: it moves the same way. Alt+click on a button still presses it
- [ ] Release, quit VEX from the tray and start it again: the island is where you left it
- [ ] Settings > Island > Reset position (and tray > Reset island position) puts it back in the centre
- [ ] Expand the island near the left or right edge: the panel stays fully on screen; collapsing returns the pill to its saved position
- [ ] Change the display resolution or scaling (Settings > Display) with VEX running: the island stays on screen and on the top edge without restarting VEX
- [ ] With two monitors, choose each display in Settings > Island: the island stays inside the chosen monitor, and dragging cannot leave it
- [ ] Lock/sleep and wake the PC: the island is still on the top edge at the saved position
- [ ] Lyrics, music controls, the assistant, the screenshot toast and the volume-free tabs all still look and animate as before while the island is off-centre

Known limits: the position is stored as a distance from the screen centre in logical pixels (clamped to about 4000), so on a very wide monitor above that the outer edges cannot be reached; the island moves only on the monitor chosen in Settings (dragging to another monitor is not supported); the keyboard path for moving it is the Settings slider; the display-change observer is untested on real hardware.

## Auto-Hide (desktop controls, stage 2)

Built and tested in the sandbox (Rust core tests, vitest, Playwright against a simulated backend):
- Settings: Always Visible is the default; Auto-Hide with a hide delay (200 ms to 5 s, default 800 ms), persisted and clamped. Settings > Island > Visibility.
- Rules (unit-tested): the island hides only in Auto-Hide mode and only when the pointer is off it, it is collapsed and not pinned, no text field is focused, no screenshot confirmation is showing, it is not being dragged and the assistant is not listening, thinking or speaking. After a reveal it stays at least 500 ms (no flicker).
- Reveals: the top-edge strip, the global shortcut, tray > Show VEX, a screenshot confirmation. Notifications, timers and widget updates deliberately do not reveal it (tested).
- Always Visible schedules no timers at all; switching to it while hidden reveals the island at once.
- Hidden state: the island window shrinks to a 4 px strip at the top of the chosen monitor, centred on the saved position; animations are paused.

Needs your laptop (Windows code is only compiled by CI here, never run):
- [ ] Settings > Island > Visibility > Auto-hide: after the delay the island slides away, leaving nothing visible on screen
- [ ] Move the pointer to the very top edge above where the island sits: it slides back. This is the main hardware risk: a transparent window strip might not receive the pointer. If it does not, tell me and I will add the fallback (cursor check, only while hidden)
- [ ] Revealing does not steal focus from the app you were typing in
- [ ] The shortcut and tray > Show VEX reveal it; a notification, a Windows toast or a timer does not
- [ ] While hidden, a screenshot (any mode) reveals the confirmation
- [ ] It stays while the pointer is over it, while expanded or pinned, while typing to the assistant, and while the assistant is busy
- [ ] No flicker when brushing the top edge; always-on-top still holds over full-screen-ish windows
- [ ] With the island moved off-centre, the strip and the reveal are at the same horizontal position
- [ ] Two monitors: the strip is at the top of the chosen monitor only
- [ ] Sleep/wake and a resolution change while hidden: it stays hidden and is still reachable at the top edge
- [ ] Switching back to Always visible brings it back immediately

Known limits: the reveal depends on the strip receiving the pointer on real Windows (untested); there is no cursor polling yet; with the pointer exactly on the strip but outside the island's width the strip is at least 280 px wide, so the reveal zone can be a little wider than a collapsed pill.

## Windows volume (desktop controls, stage 3)

Built and tested in the sandbox (Rust core tests, vitest, Playwright against a simulated backend):
- Rules (Rust and TypeScript, same cases): level rounding to whole percent (including NaN and out-of-range values from a driver), clamping before anything reaches Windows, stepping that stops at 0 and 100 with a bounded step, raising the level unmutes but 0 does not, icon level for muted/low/medium/high.
- Settings: `island.volumeButton` (default on) and `island.volumeStep` (default 5, range 1-20), persisted and clamped. The Settings screen for them arrives in stage 4.
- Island: a speaker button in the expanded island only (never in the collapsed pill), opening a compact panel with a slider, percentage, minus, plus and mute. Slider traffic is throttled to about 30 per second and always ends on the final value; echoes are ignored while dragging; external changes arrive as events; no device disables the controls with a message and recovers; a failed call shows a message and keeps the level. The panel closes when the island collapses. Nothing is requested from Windows until the island is expanded with the button on.
- Native: Core Audio (`IAudioEndpointVolume`, event callbacks, device-change notifications) on one dedicated thread, started on first use, no polling, no PowerShell, no key simulation, no new crates.

Needs your laptop (the Core Audio code is only compiled by CI here, never run):
- [ ] Expand the island: the speaker button is next to Pin. Click it: the panel shows the same level as the Windows volume flyout
- [ ] Drag the slider: the Windows volume follows smoothly; the flyout shows the same number
- [ ] Plus/minus change by the step; mute/unmute match the Windows mute state; raising the level while muted unmutes
- [ ] Press the keyboard volume keys (or change it in Windows Settings or another app) with the panel open: the slider and percentage update by themselves
- [ ] Switch the default output (speakers, headphones, Bluetooth), unplug headphones, or disable the device: the panel re-binds, shows "No audio output device" when there is none, and recovers when one returns
- [ ] The icon on the button changes for muted / low / medium / high
- [ ] The panel does not keep the island open after you move away; Auto-Hide still works with it closed
- [ ] The panel looks right in light theme and with the Appearance presets

Known limits: master volume of the default playback device only (no per-app volume, no device switching); the device's name is not shown; the Core Audio code has not run on real hardware.

## Settings: Island section (desktop controls, stage 4)

Built and tested in the sandbox (Playwright against the simulated backend):
- Settings > Island is grouped: Position (display, horizontal position with Reset position, gap from the top edge), Visibility (Always visible / Auto-hide, hide delay shown only in Auto-hide), Behaviour (expand on hover, hover delay, collapse delay, global shortcut) and Volume (volume button on/off, volume step 1-20%, disabled while the button is off).
- Changes persist, reach the open island at once (the button appears and disappears; the step is what plus and minus do), and Reset section restores every island default.
- No new settings were added in this stage; no existing control or its test id was removed.

Needs your laptop:
- [ ] Open Settings > Island: the groups read clearly, in light and dark theme and at your display scaling
- [ ] Change each setting with the island visible and see it take effect without restarting VEX (position, Auto-hide, volume button and step)
- [ ] Reset section puts the island back at the centre, Always visible, volume button on, step 5%

## Pomodoro widget (Widgets Hub, stage 9)

Built and tested in the sandbox (vitest, Playwright against the simulated backend, Rust catalog tests):
- Cycle: Focus (25 min) then Short break (5), repeated; a Long break (15) after every 4th focus session (2-8 configurable), then a new set. Lengths are 1-180 / 1-60 / 1-120 minutes. Dots show the sessions done in the current set.
- Counting: only a focus session that ran to its end counts, on the local day it ended; "Today: N focus sessions"; the last 30 days are kept. Skip never counts. Finishing a break does not count.
- Timers: one shared timer per phase, stored with an absolute end time by the native timer engine, so a running phase keeps running when the Hub is closed, across a reload, and after sleep. A phase that ended while the Hub was closed is counted exactly once, when the Hub opens, and waits for you to press Start.
- Auto-start (off by default): starts the next phase by itself only when the end happened live with the Hub open (within 15 s); never for a phase that ended long ago.
- Controls: Start / Pause / Resume, Reset (back to the full phase), Skip, and settings (lengths, long-break interval, auto-start, chime). A running phase keeps its own length; changes apply to phases that have not started.
- Island: while a phase runs or is paused the widget publishes a one-line status ("Focus, ends 14:32"). When a phase ends the island plays a short chime if "Play a chime" is on. It never reveals a hidden island, and a stale end (VEX was closed or the PC slept) is silent.
- Damaged saved data is repaired; stray Pomodoro timers do not multiply; removing and re-adding the card keeps today's count.

Needs your laptop (the native timer scheduler and the island window are only compiled by CI here):
- [ ] Add Pomodoro from the Hub gallery, press Start, close the Hub window: the island is unaffected; reopen after a minute and the time has kept counting
- [ ] Let a short phase run out (set Focus to 1 minute in the card's settings): you hear the chime from the island, the Hub shows "Focus finished" and the break waiting
- [ ] With the island on Auto-hide and hidden, a phase end plays the chime and does NOT bring the island back
- [ ] Close the Hub before a phase ends: you still hear the chime, and reopening the Hub counts the session once
- [ ] Sleep the PC during a phase: after waking the remaining time is correct; a phase that ended during sleep is counted once and is silent
- [ ] Pin Pomodoro and check the island's Widgets tab entry opens the Hub on it
- [ ] The card looks right at both sizes, in light and dark theme

Known limits: there is no Windows desktop notification for a phase end (the chime plays from the island page; VEX's notification code only reads Windows notifications); no per-task labels or long-term statistics; the compact island does not show the countdown yet (stage 15); the chime needs the island page to be running (it always is while VEX runs). The chime and the status line have not been heard or seen on real hardware.

## Notes widget (Widgets Hub, stage 10)

Built and tested in the sandbox (vitest, Playwright against the simulated backend):
- A searchable list and a plain-text editor in one card (sizes M and L). New note, open, edit, Pin/Unpin, Delete (asks first, Keep cancels). Pinned notes first, then most recently edited. Title is the first line, the rest shows as a one-line preview, with "5 min ago" style times.
- Search: every word must appear, case-insensitive, anywhere in the note.
- Saving: a moment after each keystroke, and immediately when you leave the editor, close the Hub or remove the card; a failing save shows its message. Removing and re-adding the card keeps the notes.
- Limits: 8000 characters per note, 60 notes, and the stored document is kept well under the Hub's 512 KB cap (an edit that would overflow it is refused; shrinking is always allowed). A note you never typed into is not kept.
- Plain text only: markup-looking text is shown literally; non-Latin text and emoji round-trip. A damaged saved file is repaired (bad and duplicate entries dropped, ids never reused).
- Privacy: notes stay on this PC in the Hub's data folder and are never sent anywhere or given to the AI.

Needs your laptop:
- [ ] Add Notes from the gallery, write a note, close the Hub window, reopen it: the note is there
- [ ] Quit VEX from the tray and start it again: the notes are still there
- [ ] Type quickly in a long note: no lag, and no lost characters when you click Back straight away
- [ ] Paste a large block of text (over 8000 characters): it is cut at the limit without a crash
- [ ] The editor and list look right at both card sizes, in light and dark theme, and with Windows text scaling

Known limits: plain text only (no formatting, checklists, images or attachments); no tags or folders; no export or import yet; notes are not synced between PCs; undo is the text box's own (Ctrl+Z) and does not undo a deletion.

## World Clock widget (Widgets Hub, stage 11)

Built and tested in the sandbox (vitest with real time-zone data, Playwright with a fixed clock and a pretend Kolkata viewer):
- Your own time first ("Your time zone"), then the cities you add. A new card starts with London, New York and Tokyo. Each row shows the time, a day word ("Today", "Tomorrow", "Yesterday", or a short date), the difference from your own time ("+3h 30m", "−5h 30m", "same time", including 15- and 30-minute zones such as Nepal and India), and a sun or moon for that city's own daytime (06:00-17:59).
- Add city searches a built-in list of 70 well-known cities by name, country or zone, and does not offer one you already have; up to 12 clocks. Edit mode reorders and removes (never the local row). An empty list you chose stays empty.
- Daylight saving is applied per city and per date, in both hemispheres, including the exact switching minute (checked against real zone data).
- 12/24 hour: System, 24 h or 12 h, remembered. A 24-hour display always has two digits.
- The clocks redraw once a minute, on the minute, and only while the Hub window is visible; nothing runs when the Hub is closed. No network, no location access, nothing is sent anywhere.
- A damaged saved list is repaired (bad, unknown and duplicate zones dropped).

Needs your laptop:
- [ ] Add World clock from the gallery: your own time matches the Windows taskbar clock, and the three starter cities look right against a time-zone website you trust
- [ ] Add your own cities, close the Hub, quit and restart VEX: the list is kept
- [ ] Change the Windows time zone or the Windows 12/24 hour setting while the Hub is open and check the System option follows within a minute (or after reopening)
- [ ] Leave the Hub open across a minute boundary: all clocks advance together without flicker
- [ ] The card looks right at both sizes, in light and dark theme

Known limits: a fixed list of 70 cities (no free-text places, no GPS); labels cannot be renamed yet; no alarms, meeting planner or sunrise times; day and night is a simple 06:00-18:00 rule, not real daylight; the list depends on the time-zone data in the system's web view, which Windows keeps updated.

## Countdown widget (Widgets Hub, stage 12)

Built and tested in the sandbox (vitest, Playwright against the simulated backend):
- Six quick countdowns (1, 5, 10, 15, 30 min, 1 h) that start at once, or your own time (hours, minutes, seconds, up to 99:59:59) with an optional label; Enter in the form starts it. Bad input is refused with a reason and creates nothing.
- Several can run at once (up to 8). Each has Pause/Resume, Reset, Delete; a finished one says "Time's up" and can be restarted.
- They use the shared native timer engine with absolute end times: they keep running when the Hub is closed, after a reload, and across sleep.
- The island shows the countdown that ends first as a one-line status ("Tea, ends 14:32") while it runs, and plays the same short chime when one ends (a setting on the card; stale ends are silent). It never reveals a hidden island.

Needs your laptop:
- [ ] Start a 1-minute countdown, close the Hub, wait: you hear the chime from the island and reopening shows "Time's up"
- [ ] With the island hidden (Auto-hide), a countdown ending does not bring it back
- [ ] Start two countdowns, sleep the PC, wake it: the remaining times are correct
- [ ] Both card sizes look right in light and dark theme

Known limits: no desktop notification (only the chime); the length of a countdown cannot be edited after it is created (delete and recreate); no alarms at a clock time; no count-up stopwatch.
