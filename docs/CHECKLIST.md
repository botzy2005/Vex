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
