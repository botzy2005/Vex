# VEX Widgets Hub: framework architecture (stage 8)

Scope: the framework only. No productivity widget is built in this stage. The one built-in widget
("Getting started") exists so the framework has a real, removable occupant and the empty state is
not the only thing to test.

## 1. Principles

- **Reuse, don't replace.** Same patterns as the launcher and screenshot modules: logic in
  `vex-core` (unit-tested on Linux), thin Tauri shell, `Backend` interface with a Tauri and a test
  implementation, atomic JSON persistence in `app_config_dir`.
- **Nothing runs when the Hub is closed.** The Hub window is created on open and destroyed on close
  (the same pattern as Settings). Only the timer scheduler is resident, and it sleeps with no
  deadline when no timer is running.
- **One source of truth for what a widget is:** `src/shared/widgets.json`, read by TypeScript and
  embedded in Rust (`include_str!`). A cargo test and a vitest check both sides agree.

## 2. Widget registration and lifecycle

Catalog entry (`widgets.json`, platform-neutral facts): `id`, `title`, `description`, `sizes`
(subset of `s` 1x1, `m` 2x1, `t` 1x2, `l` 2x2), `defaultSize`. Every widget is a **singleton**: its
id is its identity, there is at most one card per kind. This keeps data keyed by kind and means
removing a card never deletes the user's data (re-adding brings it back).

UI registry (`src/hub/registry.ts`): `id -> { icon, load }`, where `load` is a dynamic `import()` so a
widget's code is fetched only when its card is first shown (lazy loading).

Lifecycle, enforced by `WidgetHost`:

1. **registered**: present in the catalog and the registry.
2. **placed**: present in the saved layout (added by the user).
3. **loaded**: the chunk is fetched when the card first renders (Suspense skeleton meanwhile).
4. **mounted / active**: the widget gets `{ size, active }`; `active` is false while the Hub is
   hidden or minimised so widgets can stop polling. Widgets must not start work when `active` is false.
5. **unmounted**: removed from the layout or the Hub window closed; effects clean up.
6. **failed**: each card has an error boundary. A crashing widget shows an inline error with
   "Reload" and "Remove"; the rest of the Hub keeps working.

Widgets get persistence through `useWidgetData(kind, initial)`, a debounced load/save of one JSON
document per kind (`hub-data/<kind>.json`, atomic, 512 KB cap, kind must be in the catalog).

## 3. Dashboard layout and persistence

`hub.json`: `{ version: 1, widgets: [{ kind, size, pinned }] }`. Order in the array is the display
order; presence means visible. Rust `sanitize` (unit-tested) drops unknown kinds and duplicates,
coerces a size that the widget does not allow back to its default, and caps the count. A corrupt file is
moved aside and the default layout (just "Getting started") is restored, never a crash.

Layout operations (`add`, `remove`, `move`, `resize`, `pin`, `reset`) are pure functions in
`src/hub/layout.ts` (vitest, and reused by the test backend so there is no second copy of the rules).
The UI applies an operation and calls `hubSaveLayout(layout)`; Rust validates and persists it and
emits `hub-changed` so the island (and a second Hub window, if ever) see it. The grid is a 4-column CSS grid
with a fixed row height and `grid-auto-flow: dense`; it reflows to 2 columns when the window is narrow.

## 4. Add, remove, pin, rearrange

- **Customise mode** (toggle in the header). Outside it, cards are calm: title, content, a pin button.
  In it, cards show a drag handle, size choices (only the sizes the widget allows) and a remove button.
- **Add**: a gallery lists catalog widgets not yet placed, with description; a widget already placed is
  shown as added. Adding appends to the layout.
- **Rearrange**: pointer-based dragging on the handle (own pointer-event code, not HTML5 drag and
  drop, which Tauri's window drop handler intercepts on Windows), plus keyboard `Alt+Left/Right` and
  explicit "move earlier / later" buttons for accessibility. The reorder maths is a pure function.
- **Pin** means "show in the island's Widgets tab for one-click access" (max 4 pinned). It does not
  change the order.
- **Reset layout** restores the default.

## 5. Shared timer infrastructure (`vex_core::timers`)

Needed by Pomodoro and Countdown, so it is built and tested once, now.

- A timer is `{ id, owner (a widget kind), label, duration_ms, state }` with state
  `idle | running{ends_at} | paused{remaining} | finished{at}`. **Running timers store an absolute
  wall-clock end time (unix ms)**, never a tick count, so the remaining time is always
  `ends_at - now`. That is correct after the window closes, after the PC sleeps, and after a restart.
- The engine is pure: every operation takes `now_ms`, so tests use a fake clock (including jumps that
  simulate sleep). Operations: create, start, pause, resume, reset, delete, set duration; `poll(now)`
  returns timers that have just finished, each reported exactly once. Limits: 50 timers, 1 s to 366 days,
  label 80 chars, owner must be in the catalog.
- Persisted to `timers.json` (atomic). On startup, timers that ended while VEX was closed become
  `finished` and are reported once ("finished while VEX was closed").
- **Scheduler** (`src-tauri/src/hub.rs`): one thread blocked on a `Condvar`. With no running timer it
  waits indefinitely (zero CPU). With running timers it waits until the earliest end, capped at 30 s so a
  sleep/wake or clock change is corrected within 30 s, then re-reads the wall clock. Operations wake it.
- Events: `timers-changed` (full list, small) and `timer-finished { id, owner, label }`.
- UI: the view carries absolute times; `useTimers()` re-renders once per second **only** while a timer is
  running and the consumer is `active`. Alerts (sound, island state) are the consumers' job; a desktop
  toast is not part of this stage because the existing notification code only *reads* Windows
  notifications. That decision is made in the Pomodoro stage.

## 6. Widgets and the Dynamic Island

Three small, explicit channels. The island never imports widget code.

1. **Layout** (`hub-changed`): the island's new **Widgets tab** lists pinned widgets (icon, title) and an
   "Open Hub" button. A pinned entry opens the Hub focused on that widget.
2. **Timers** (`timers-changed`, `timer-finished`): the island can show a running/finished timer from the
   shared list with no widget code loaded. (Compact-island indicators are stage 15; the channel and hook
   exist now.)
3. **Status** (`hub_set_status(kind, status | null)` -> `hub-status`): a widget may publish one short
   in-memory line (`label`, optional `detail`, `priority`) for the island to show contextually. Never
   persisted, cleared when the Hub closes, text length-limited, kind validated.

Entry points: the island's expanded header gets a Widgets tab; the tray gets "Widgets Hub"; the command
is `hub_open(widget?)`. No new global shortcut (avoids more clashes); can be added in Settings later.

## 7. Security and privacy

Widget data and timers never leave the PC and are never given to the AI. All commands take typed
arguments, validate `kind` against the catalog and bound sizes; nothing builds shell strings. No new
dependencies.

## 8. Testing

- Rust: layout sanitise, catalog parity, data-store limits and corrupt recovery, timer engine (fake clock,
  sleep jump, restart, exactly-once finish, limits).
- vitest: layout operations, reorder maths, timer formatting/remaining, catalog parity with Rust.
- Playwright (test backend): open/empty state, add, remove, resize, pin limit, reorder by pointer and by
  keyboard, persistence across reload, lazy loading, error boundary isolation, island Widgets tab.
- Windows code (window creation, tray item, scheduler thread) compiles only in CI and has not been run on a
  real PC; the report will say so.
