/**
 * TEST HARNESS: NOT PART OF THE SHIPPED APP.
 *
 * Browser stand-in for the Widgets Hub's native side. The real rules (layout validation, data limits,
 * the timer engine) live in Rust (`vex_core::hub`, unit-tested there); this keeps just enough behaviour
 * for the UI tests. State survives reloads and is shared between pages through localStorage plus a
 * BroadcastChannel, the way the native side is shared between windows.
 */
import { registerTestWidgets, TEST_WIDGETS } from '../hub/test_widgets';
import type { HubLayout, HubStatus, HubStatusEntry, HubTimer, TimerFinished, WidgetDef } from '../shared/types';
import widgets from '../shared/widgets.json';
import type { Backend, Unsub } from './backend';

const LAYOUT_KEY = 'islet-test-hub-layout';
const DATA_KEY = 'islet-test-hub-data:';
const TIMERS_KEY = 'islet-test-hub-timers';
const MAX_PINNED = 4;

export interface HubHarness {
  /** Replace the saved layout (as if it had been saved earlier) and tell every page. */
  setLayout(l: HubLayout): void;
  layout(): HubLayout;
  /** Forget layout, data, timers and statuses. */
  reset(): void;
  /** Every layout the page asked to save, in order. */
  readonly saves: HubLayout[];
  readonly opens: Array<string | undefined>;
  failNextSave(message: string | null): void;
  failNextData(message: string | null): void;
  /** Raw saved document of one widget. */
  data(kind: string): unknown;
  readonly dataWrites: Array<{ kind: string; value: unknown }>;
  statuses(): HubStatusEntry[];
  /** Pretend another window changed the layout. */
  emitLayout(l: HubLayout): void;
  /** The stand-in's backend calls, so a test can publish a status or create a timer as a widget would. */
  api: HubApi;
}

export type HubApi = Pick<
  Backend,
  | 'hubOpen' | 'hubCatalog' | 'hubLayout' | 'hubSaveLayout' | 'onHubChanged' | 'onHubFocus' | 'hubDataGet' | 'hubDataSet'
  | 'hubSetStatus' | 'hubStatuses' | 'onHubStatus' | 'timersList' | 'timerCreate' | 'timerStart' | 'timerPause'
  | 'timerReset' | 'timerDelete' | 'timerSetDuration' | 'timerSetLabel' | 'onTimers' | 'onTimerFinished'
>;

export function createTestHub() {
  registerTestWidgets();
  const catalog: WidgetDef[] = [...(widgets.widgets as WidgetDef[]), ...TEST_WIDGETS];
  const known = (k: string) => catalog.some((d) => d.id === k);

  const layoutSubs = new Set<(l: HubLayout) => void>();
  const focusSubs = new Set<(k: string) => void>();
  const statusSubs = new Set<(s: HubStatusEntry[]) => void>();
  const timerSubs = new Set<(t: HubTimer[]) => void>();
  const finishedSubs = new Set<(f: TimerFinished) => void>();
  const channel = new BroadcastChannel('islet-test-hub');

  const read = <T,>(key: string): T | null => {
    try {
      const raw = localStorage.getItem(key);
      return raw ? (JSON.parse(raw) as T) : null;
    } catch {
      return null;
    }
  };
  const write = (key: string, v: unknown) => localStorage.setItem(key, JSON.stringify(v));

  // ---- layout
  const initial = (): HubLayout => ({ version: 1, widgets: [{ kind: 'welcome', size: 'm', pinned: false }] });
  const sanitize = (l: HubLayout): HubLayout => {
    const seen = new Set<string>();
    let pinned = 0;
    const out = l.widgets
      .filter((p) => known(p.kind) && !seen.has(p.kind) && seen.add(p.kind))
      .map((p) => {
        const def = catalog.find((d) => d.id === p.kind)!;
        const size = def.sizes.includes(p.size) ? p.size : def.defaultSize;
        const pin = p.pinned && pinned < MAX_PINNED;
        if (pin) pinned++;
        return { kind: p.kind, size, pinned: !!pin };
      });
    return { version: 1, widgets: out };
  };
  const currentLayout = (): HubLayout => {
    const l = read<HubLayout>(LAYOUT_KEY);
    return l ? sanitize(l) : initial();
  };

  const saves: HubLayout[] = [];
  const opens: Array<string | undefined> = [];
  const dataWrites: Array<{ kind: string; value: unknown }> = [];
  let failSave: string | null = null;
  let failData: string | null = null;
  let statuses = new Map<string, HubStatus>();

  const emitLayout = (l: HubLayout) => layoutSubs.forEach((cb) => cb(l));
  const statusList = (): HubStatusEntry[] => [...statuses].map(([kind, s]) => ({ kind, ...s }));
  const emitStatus = () => statusSubs.forEach((cb) => cb(statusList()));

  // ---- timers (absolute end times, like the native engine)
  let timerSeq = 0;
  let timerWake: number | undefined;
  const timers = (): HubTimer[] => read<HubTimer[]>(TIMERS_KEY) ?? [];
  const saveTimers = (t: HubTimer[]) => {
    write(TIMERS_KEY, t);
    channel.postMessage({ type: 'timers', list: t });
    timerSubs.forEach((cb) => cb(t));
    schedule();
  };
  const schedule = () => {
    window.clearTimeout(timerWake);
    const next = timers().reduce<number | null>((m, t) => (t.state === 'running' ? Math.min(m ?? Infinity, t.endsAt) : m), null);
    if (next === null) return;
    timerWake = window.setTimeout(poll, Math.max(0, next - Date.now()) + 5);
  };
  const poll = () => {
    const now = Date.now();
    const list = timers();
    const done: TimerFinished[] = [];
    const next = list.map((t): HubTimer => {
      if (t.state === 'running' && t.endsAt <= now) {
        done.push({ id: t.id, owner: t.owner, label: t.label, lateMs: now - t.endsAt });
        return { id: t.id, owner: t.owner, label: t.label, durationMs: t.durationMs, state: 'finished', at: t.endsAt };
      }
      return t;
    });
    if (done.length) {
      saveTimers(next);
      done.forEach((f) => finishedSubs.forEach((cb) => cb(f)));
    } else schedule();
  };
  const mutate = (id: number, f: (t: HubTimer) => HubTimer) => {
    poll();
    const list = timers();
    const i = list.findIndex((t) => t.id === id);
    if (i < 0) throw new Error('That timer no longer exists.');
    list[i] = f(list[i]);
    saveTimers(list);
  };

  channel.onmessage = (e) => {
    const m = e.data as { type: string; layout?: HubLayout; kind?: string; list?: unknown };
    if (m.type === 'layout') emitLayout(m.layout!);
    else if (m.type === 'focus') focusSubs.forEach((cb) => cb(m.kind!));
    else if (m.type === 'status') {
      statuses = new Map((m.list as HubStatusEntry[]).map((s) => [s.kind, { label: s.label, detail: s.detail, priority: s.priority }]));
      emitStatus();
    } else if (m.type === 'timers') {
      timerSubs.forEach((cb) => cb(m.list as HubTimer[]));
      schedule();
    }
  };
  schedule();

  const sub = <T,>(set: Set<(v: T) => void>, cb: (v: T) => void): Promise<Unsub> => {
    set.add(cb);
    return Promise.resolve(() => void set.delete(cb));
  };

  const harness: HubHarness = {
    setLayout(l) {
      const kept = sanitize(l);
      write(LAYOUT_KEY, kept);
      channel.postMessage({ type: 'layout', layout: kept });
      emitLayout(kept);
    },
    layout: currentLayout,
    reset() {
      localStorage.removeItem(LAYOUT_KEY);
      localStorage.removeItem(TIMERS_KEY);
      for (const k of Object.keys(localStorage)) if (k.startsWith(DATA_KEY)) localStorage.removeItem(k);
      statuses = new Map();
      saves.length = 0;
      opens.length = 0;
      dataWrites.length = 0;
      failSave = null;
      failData = null;
      window.__probe = { loads: [], mounts: {}, unmounts: {}, active: {} };
      window.clearTimeout(timerWake);
      channel.postMessage({ type: 'layout', layout: initial() });
      channel.postMessage({ type: 'status', list: [] });
      channel.postMessage({ type: 'timers', list: [] });
      emitLayout(initial());
      emitStatus();
      timerSubs.forEach((cb) => cb([]));
    },
    saves,
    opens,
    failNextSave: (m) => void (failSave = m),
    failNextData: (m) => void (failData = m),
    data: (kind) => read(DATA_KEY + kind),
    dataWrites,
    statuses: statusList,
    emitLayout,
    api: undefined as unknown as HubApi,
  };

  const api: HubApi = {
    hubOpen: async (widget) => {
      opens.push(widget);
      if (widget) channel.postMessage({ type: 'focus', kind: widget });
    },
    hubCatalog: async () => catalog,
    hubLayout: async () => currentLayout(),
    hubSaveLayout: async (layout) => {
      saves.push(structuredClone(layout));
      if (failSave) {
        const m = failSave;
        failSave = null;
        throw m;
      }
      const kept = sanitize(layout);
      write(LAYOUT_KEY, kept);
      channel.postMessage({ type: 'layout', layout: kept });
      return kept;
    },
    onHubChanged: (cb) => sub(layoutSubs, cb),
    onHubFocus: (cb) => sub(focusSubs, cb),
    hubDataGet: async (kind) => {
      if (!known(kind)) throw 'That widget does not exist.';
      return read(DATA_KEY + kind);
    },
    hubDataSet: async (kind, value) => {
      if (!known(kind)) throw 'That widget does not exist.';
      if (failData) {
        const m = failData;
        failData = null;
        throw m;
      }
      if (JSON.stringify(value).length > 512 * 1024) throw 'That is too much data for one widget (limit 512 KB).';
      dataWrites.push({ kind, value: structuredClone(value) });
      write(DATA_KEY + kind, value);
    },
    hubSetStatus: async (kind, status) => {
      if (!known(kind)) throw 'That widget does not exist.';
      if (status) {
        const label = status.label.replace(/[\u0000-\u001f]/g, '').trim().slice(0, 40);
        if (!label) throw 'A status needs a label.';
        statuses.set(kind, { label, detail: status.detail?.trim().slice(0, 80) || null, priority: Math.min(3, Math.max(0, status.priority | 0)) });
      } else statuses.delete(kind);
      channel.postMessage({ type: 'status', list: statusList() });
      emitStatus();
    },
    hubStatuses: async () => statusList(),
    onHubStatus: (cb) => sub(statusSubs, cb),
    timersList: async () => timers(),
    timerCreate: async (owner, label, durationMs) => {
      if (!known(owner)) throw 'That widget does not exist.';
      if (durationMs < 1000 || durationMs > 366 * 86400 * 1000) throw 'A timer must last between 1 second and 366 days.';
      const list = timers();
      const id = Math.max(timerSeq, ...list.map((t) => t.id)) + 1;
      timerSeq = id;
      saveTimers([...list, { id, owner, label: label.trim().slice(0, 80), durationMs, state: 'idle' }]);
      return id;
    },
    timerStart: async (id) =>
      mutate(id, (t) => {
        const base = { id: t.id, owner: t.owner, label: t.label, durationMs: t.durationMs };
        if (t.state === 'running') return t;
        if (t.state === 'paused') return { ...base, state: 'running', endsAt: Date.now() + t.remainingMs };
        return { ...base, state: 'running', endsAt: Date.now() + t.durationMs };
      }),
    timerPause: async (id) =>
      mutate(id, (t) =>
        t.state === 'running' ? { id: t.id, owner: t.owner, label: t.label, durationMs: t.durationMs, state: 'paused', remainingMs: Math.max(0, t.endsAt - Date.now()) } : t,
      ),
    timerReset: async (id) => mutate(id, (t) => ({ id: t.id, owner: t.owner, label: t.label, durationMs: t.durationMs, state: 'idle' })),
    timerDelete: async (id) => {
      const list = timers();
      if (!list.some((t) => t.id === id)) throw 'That timer no longer exists.';
      saveTimers(list.filter((t) => t.id !== id));
    },
    timerSetDuration: async (id, durationMs) =>
      mutate(id, (t) => {
        if (t.state === 'running') throw 'Pause or reset the timer before changing its length.';
        return { id: t.id, owner: t.owner, label: t.label, durationMs, state: 'idle' };
      }),
    timerSetLabel: async (id, label) => mutate(id, (t) => ({ ...t, label: label.trim().slice(0, 80) })),
    onTimers: (cb) => sub(timerSubs, cb),
    onTimerFinished: (cb) => sub(finishedSubs, cb),
  };

  harness.api = api;
  return { api, harness };
}
