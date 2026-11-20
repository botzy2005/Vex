/**
 * TEST HARNESS — NOT PART OF THE SHIPPED APP.
 *
 * An in-browser stand-in for the native backend so the real UI code can be
 * exercised by `npm run dev` and the automated browser tests. The media
 * "session" here is a simulated external player that the tests drive through
 * `window.__harness`; the production app never contains this file.
 */
import defaults from '../shared/defaults.json';
import { mergePatch } from '../shared/merge';
import type {
  AiEvent,
  BenchRow,
  MetricsView,
  AiTestResult,
  AssistantState,
  ChatMessage,
  ContextPayload,
  Conversation,
  ConversationSummary,
  LyricsFetchResult,
  LyricsQuery,
  MediaSession,
  MediaState,
  NotificationAccess,
  NotificationsView,
  ClipboardItem,
  ClipboardStatus,
  CaptureMode,
  ShotSource,
  ShotView,
  Settings,
  SettingsPatch,
  TransportCommand,
} from '../shared/types';
import type { Backend, Unsub } from './backend';

const KEY = 'islet-test-settings';

export interface AiScript {
  chunks?: string[];
  delayMs?: number;
  /** Make the reply fail after the chunks (or immediately when there are none). */
  error?: { message: string; kind?: string };
  /** Silence before anything arrives, like a model that is still loading. */
  silentMs?: number;
  /** Then hidden reasoning for this long, reported as 'thinking' events. */
  thinkingMs?: number;
}

export interface Harness {
  /** Replace (or clear) the simulated external media session. */
  setSession(s: Partial<MediaSession> | null): void;
  /** What the simulated lyrics service answers: a result, null (no lyrics), or an error message. */
  setLyricsReply(reply: LyricsFetchResult | null | { error: string }): void;
  readonly lyricsCalls: LyricsQuery[];
  /** Apply a settings patch exactly as the Settings window would. */
  patchSettings: (patch: SettingsPatch) => Promise<void>;
  emitAssistant(state: AssistantState): void;
  emitIslandToggle(): void;
  readonly transportCalls: TransportCommand[];
  readonly windowSizes: Array<[number, number]>;
  /** Script the simulated model: reply chunks, per-chunk delay, or an error. */
  setAiScript(script: AiScript): void;
  /** Seed the simulated measurements shown on the Developer page. */
  setMetrics(v: MetricsView): void;
  /** Rows the simulated benchmark will report (one per call, in order). */
  setBenchRows(rows: BenchRow[]): void;
  readonly benchCalls: Array<{ models: string[]; includeThinking: boolean }>;
  readonly preloadCalls: string[];
  /** Simulated Ollama service: whether it runs and which models it has installed. */
  setOllama(o: { running: boolean; models: string[] }): void;
  /** Keys the simulated provider accepts (anything else is rejected as invalid). */
  setValidKeys(keys: string[]): void;
  readonly aiSends: Array<{ conversationId: string | null; text: string; context: ContextPayload | null; provider: string; model: string }>;
  /** Forget everything in memory, as if the app restarted (history stays in storage). */
  restartAi(): void;
  readonly settingsOpened: number;
  /** Simulate Windows delivering a notification (kept only when the module is on and the app is not blocked). */
  notify(n: { appId: string; appName?: string; title?: string; body?: string; icon?: string | null; tsMs?: number }): void;
  /** Force what the simulated Windows says about permission (null = allowed while the module is on). */
  setNotificationAccess(a: NotificationAccess | null): void;
  readonly notificationOpens: number[];
  /** Simulate another program copying text or an image (recorded only while the module is on and not paused). */
  clipboardCopyExternal(c: { kind: 'text'; text: string } | { kind: 'image'; seed?: number; sensitive?: boolean }): void;
  /** What a "copy back" last put on the simulated clipboard. */
  readonly clipboardWrites: Array<{ kind: 'text' | 'image'; text?: string; id: number }>;
  /** Make the next copy-back fail with this message. */
  failNextClipboardCopy(message: string | null): void;
  /** Every screenshot action this page asked for, in order (for assertions). */
  readonly screenshotCalls: Array<Record<string, unknown>>;
  /** Simulate a finished capture (all pages see it). Returns its id. */
  addShot(o?: { w?: number; h?: number }): number;
  /** Make the next screenshot action fail with this message. */
  failNextScreenshot(message: string | null): void;
  /** What "Save" returns: a path, or null as if the user cancelled the dialog. */
  setSavePath(path: string | null): void;
  /** Drop every stored simulated capture (as if VEX restarted with history off). */
  resetShots(): void;
}

declare global {
  interface Window {
    __harness?: Harness;
  }
}

export function createTestBackend(): Backend {
  let settings: Settings = structuredClone(defaults) as Settings;
  try {
    const saved = localStorage.getItem(KEY);
    if (saved) settings = mergePatch(settings, JSON.parse(saved));
  } catch {
    /* ignore */
  }

  const settingsSubs = new Set<(s: Settings) => void>();
  const mediaSubs = new Set<(s: MediaState) => void>();
  const assistantSubs = new Set<(s: AssistantState) => void>();
  const toggleSubs = new Set<() => void>();
  let session: MediaSession | null = null;

  const calls: TransportCommand[] = [];
  const sizes: Array<[number, number]> = [];
  const aiSends: Harness['aiSends'] = [];
  let metrics: MetricsView = { requests: [], requestsSent: 0 };
  let benchRows: BenchRow[] = [];
  const benchCalls: Harness['benchCalls'] = [];
  const preloadCalls: string[] = [];
  const benchSubs = new Set<(r: BenchRow) => void>();
  let script: AiScript = { chunks: ['Hello', ' from ', 'the test model.'], delayMs: 30 };
  let ollama = { running: true, models: ['llama3.2:latest', 'qwen2.5:0.5b'] };
  let validKeys = ['good-key-123'];
  const keys = new Map<string, string>();
  const aiSubs = new Set<(e: AiEvent) => void>();
  let activeCancel: { cancelled: boolean } | null = null;
  let nextRequest = 1;
  const CHATS = 'vex-test-chats';
  const loadChats = (): Conversation[] => {
    try {
      return JSON.parse(localStorage.getItem(CHATS) ?? '[]');
    } catch {
      return [];
    }
  };
  const saveChats = (c: Conversation[]) => {
    try {
      localStorage.setItem(CHATS, JSON.stringify(c));
    } catch {
      /* ignore */
    }
  };
  const emitAi = (e: AiEvent) => aiSubs.forEach((cb) => cb(e));
  let opened = 0;
  let lyricsReply: LyricsFetchResult | null | { error: string } = null;
  const lyricsCalls: LyricsQuery[] = [];

  // ---- simulated Windows notifications (stand-in for the native listener; the real rules live in Rust)
  const notifChannel = new BroadcastChannel('vex-test-notifications');
  const notifSubs = new Set<(v: NotificationsView) => void>();
  let nview: NotificationsView = { access: { state: 'idle' }, entries: [], apps: [], unread: 0 };
  let nextNid = 1;
  let accessOverride: NotificationAccess | null = null;
  const notificationOpens: number[] = [];
  const publishN = (broadcast = true) => {
    const n = settings.notifications;
    const access: NotificationAccess = n.enabled ? accessOverride ?? { state: 'allowed' } : { state: 'idle' };
    const entries = nview.entries;
    nview = {
      access,
      entries,
      apps: nview.apps.map((a) => ({ ...a, blocked: n.blockedApps.includes(a.id) })),
      unread: entries.filter((e) => e.unread).length,
    };
    notifSubs.forEach((cb) => cb(nview));
    if (broadcast) notifChannel.postMessage({ type: 'view', view: nview });
  };
  /** The same rules the Rust centre applies when settings change. */
  const reconfigureN = () => {
    const n = settings.notifications;
    let { entries, apps } = nview;
    if (!n.enabled) {
      entries = [];
      apps = [];
    }
    entries = entries.filter((e) => !n.blockedApps.includes(e.appId));
    if (!n.showPreviews) entries = entries.map((e) => ({ ...e, title: '', body: '' }));
    entries = entries.slice(0, n.historyLimit);
    nview = { ...nview, entries, apps };
    publishN();
  };
  notifChannel.onmessage = (e) => {
    const d = e.data as { type: string; view?: NotificationsView };
    if (d.type === 'view' && d.view) {
      nview = d.view;
      notifSubs.forEach((cb) => cb(nview));
    } else if (d.type === 'hello' && (nview.entries.length || nview.apps.length)) notifChannel.postMessage({ type: 'view', view: nview });
  };
  notifChannel.postMessage({ type: 'hello' });

  // ---- simulated clipboard history (stand-in for the native engine; the real rules live in Rust)
  type SimClip = ClipboardItem & { text: string; hash: string };
  const clipSubs = new Set<(s: ClipboardStatus) => void>();
  let clipItems: SimClip[] = []; // oldest first
  let nextCid = 1;
  let skippedSensitive = 0;
  let clipNote: string | null = null;
  const clipboardWrites: Array<{ kind: 'text' | 'image'; text?: string; id: number }> = [];
  let failCopy: string | null = null;
  const clipStatus = (): ClipboardStatus => {
    const c = settings.clipboard;
    return {
      enabled: c.enabled,
      paused: c.paused,
      recording: c.enabled && !c.paused,
      count: clipItems.length,
      pinned: clipItems.filter((i) => i.pinned).length,
      imageBytes: clipItems.reduce((n, i) => n + i.bytes, 0),
      skippedSensitive,
      note: clipNote,
      persist: c.persist,
    };
  };
  // The island and the settings window are separate pages here; they share the simulated history over a channel
  // (the real app has one history in Rust that both windows ask).
  const clipChannel = new BroadcastChannel('vex-test-clipboard');
  const publishClip = (broadcast = true) => {
    clipSubs.forEach((cb) => cb(clipStatus()));
    if (broadcast) clipChannel.postMessage({ type: 'state', items: clipItems, skippedSensitive, note: clipNote, nextId: nextCid });
  };
  const trimClip = () => {
    const max = settings.clipboard.maxItems;
    // Oldest unpinned entries go first; pinned ones are never trimmed.
    while (clipItems.length > max) {
      const i = clipItems.findIndex((x) => !x.pinned);
      if (i < 0) break;
      clipItems.splice(i, 1);
    }
  };
  const reconfigureClip = () => {
    const c = settings.clipboard;
    if (!c.enabled) clipItems = [];
    if (!c.images) clipItems = clipItems.filter((i) => i.kind !== 'image');
    trimClip();
    publishClip();
  };
  clipChannel.onmessage = (e) => {
    const d = e.data as { type: string; items?: SimClip[]; skippedSensitive?: number; note?: string | null; nextId?: number };
    if (d.type === 'state' && d.items) {
      clipItems = d.items;
      skippedSensitive = d.skippedSensitive ?? 0;
      clipNote = d.note ?? null;
      nextCid = Math.max(nextCid, d.nextId ?? 1);
      publishClip(false);
    } else if (d.type === 'hello' && (clipItems.length || skippedSensitive)) publishClip();
  };
  clipChannel.postMessage({ type: 'hello' });
  const addClip = (item: Omit<SimClip, 'id' | 'tsMs' | 'pinned'>) => {
    const last = clipItems[clipItems.length - 1];
    if (last && last.hash === item.hash) return;
    const existing = clipItems.findIndex((i) => i.hash === item.hash);
    if (existing >= 0) {
      const [it] = clipItems.splice(existing, 1);
      clipItems.push({ ...it, tsMs: Date.now() });
    } else clipItems.push({ ...item, id: nextCid++, tsMs: Date.now(), pinned: false });
    trimClip();
    publishClip();
  };

  // ---- simulated screenshots (stand-in for native capture; the real rules live in Rust)
  // The island, the overlay, the editor and settings are separate pages here, so the "history" lives in
  // localStorage and captures are announced over a channel (the real app has one history in Rust).
  const SHOTS_KEY = 'vex-test-shots';
  type SimShot = { view: ShotView; data: string };
  const shotChannel = new BroadcastChannel('vex-test-shots');
  const shotCapturedSubs = new Set<(s: ShotView) => void>();
  const shotErrorSubs = new Set<(m: string) => void>();
  const editorLoadSubs = new Set<(id: number) => void>();
  const screenshotCalls: Array<Record<string, unknown>> = [];
  let failShot: string | null = null;
  let savePath: string | null = 'C:\\Users\\you\\Pictures\\VEX Screenshots\\VEX Screenshot 2026-11-18 10.00.00.png';
  const loadShots = (): { next: number; items: SimShot[] } => {
    try {
      return JSON.parse(localStorage.getItem(SHOTS_KEY) ?? '') as { next: number; items: SimShot[] };
    } catch {
      return { next: 1, items: [] };
    }
  };
  const saveShots = (v: { next: number; items: SimShot[] }) => {
    try {
      localStorage.setItem(SHOTS_KEY, JSON.stringify(v));
    } catch {
      /* ignore */
    }
  };
  /** A picture with known pixels: left half red, right half blue, a black square top-left and a stripe patch top-right. */
  const makeShot = (w: number, h: number): string => {
    const c = document.createElement('canvas');
    c.width = w;
    c.height = h;
    const g = c.getContext('2d')!;
    g.fillStyle = '#ff0000';
    g.fillRect(0, 0, w / 2, h);
    g.fillStyle = '#0000ff';
    g.fillRect(w / 2, 0, w / 2, h);
    g.fillStyle = '#000000';
    g.fillRect(0, 0, 10, 10);
    for (let i = 0; i < 20; i++) {
      g.fillStyle = i % 2 ? '#ffffff' : '#000000';
      g.fillRect(w - 120 + i * 5, 20, 5, 40);
    }
    return c.toDataURL('image/png');
  };
  const thumbOf = (data: string) => data; // small test images double as their own thumbnails
  const historyLimit = () => ({ off: 1, session: 10, keep: 30 })[settings.screenshot.history];
  const addShot = (w: number, h: number): ShotView => {
    const store = loadShots();
    const data = makeShot(w, h);
    const view: ShotView = { id: store.next, tsMs: Date.now(), width: w, height: h, bytes: data.length, thumb: thumbOf(data) };
    const items = [...store.items, { view, data }].slice(-historyLimit());
    saveShots({ next: store.next + 1, items });
    shotCapturedSubs.forEach((cb) => cb(view));
    shotChannel.postMessage({ type: 'captured', view });
    return view;
  };
  shotChannel.onmessage = (e) => {
    const d = e.data as { type: string; view?: ShotView; message?: string; id?: number };
    if (d.type === 'captured' && d.view) shotCapturedSubs.forEach((cb) => cb(d.view!));
    else if (d.type === 'error' && d.message) shotErrorSubs.forEach((cb) => cb(d.message!));
    else if (d.type === 'editor-load' && d.id !== undefined) editorLoadSubs.forEach((cb) => cb(d.id!));
  };
  const shotFail = () => {
    if (failShot) {
      const m = failShot;
      failShot = null;
      throw new Error(m);
    }
  };

  const sub = <T,>(set: Set<(v: T) => void>, cb: (v: T) => void): Promise<Unsub> => {
    set.add(cb);
    return Promise.resolve(() => void set.delete(cb));
  };
  const emitMedia = () => mediaSubs.forEach((cb) => cb({ session }));
  const nowPos = (s: MediaSession) =>
    s.status === 'playing' ? s.positionMs + (Date.now() - s.positionUpdatedAt) * s.playbackRate : s.positionMs;

  const channel = new BroadcastChannel('islet-test-assistant');
  channel.onmessage = (e) => assistantSubs.forEach((cb) => cb(e.data as AssistantState));

  window.addEventListener('storage', (e) => {
    if (e.key === KEY && e.newValue) {
      settings = mergePatch(structuredClone(defaults) as Settings, JSON.parse(e.newValue));
      settingsSubs.forEach((cb) => cb(settings));
      reconfigureClip();
    }
  });

  const harness: Harness = {
    patchSettings: async () => {},
    setSession(partial) {
      if (partial === null) session = null;
      else {
        session = {
          appId: 'test-player',
          title: 'Test Track',
          artist: 'Test Artist',
          durationMs: 200_000,
          positionMs: 0,
          positionUpdatedAt: Date.now(),
          playbackRate: 1,
          status: 'playing',
          capabilities: { play: true, pause: true, next: true, previous: true, seek: true },
          ...partial,
        };
        if (partial.positionMs !== undefined && partial.positionUpdatedAt === undefined) {
          session.positionUpdatedAt = Date.now();
        }
      }
      emitMedia();
    },
    setLyricsReply: (r) => {
      lyricsReply = r;
    },
    lyricsCalls,
    emitAssistant: (s) => assistantSubs.forEach((cb) => cb(s)),
    emitIslandToggle: () => toggleSubs.forEach((cb) => cb()),
    transportCalls: calls,
    windowSizes: sizes,
    setAiScript: (sc) => {
      script = sc;
    },
    setOllama: (o) => {
      ollama = o;
    },
    setValidKeys: (k) => {
      validKeys = k;
    },
    setMetrics: (v) => {
      metrics = v;
    },
    setBenchRows: (rows) => {
      benchRows = rows;
    },
    benchCalls,
    preloadCalls,
    aiSends,
    restartAi: () => {
      activeCancel = null;
    },
    get settingsOpened() {
      return opened;
    },
    notify(n) {
      const cfg = settings.notifications;
      if (!cfg.enabled || cfg.blockedApps.includes(n.appId)) return;
      const entry = {
        id: nextNid++,
        appId: n.appId,
        appName: n.appName ?? n.appId,
        title: cfg.showPreviews ? n.title ?? '' : '',
        body: cfg.showPreviews ? n.body ?? '' : '',
        tsMs: n.tsMs ?? Date.now(),
        unread: true,
      };
      const apps = nview.apps.some((a) => a.id === n.appId)
        ? nview.apps.map((a) => (a.id === n.appId ? { ...a, name: entry.appName, icon: n.icon ?? a.icon } : a))
        : [...nview.apps, { id: n.appId, name: entry.appName, icon: n.icon ?? null, blocked: false }];
      nview = { ...nview, apps, entries: [entry, ...nview.entries].slice(0, cfg.historyLimit) };
      publishN();
    },
    setNotificationAccess(a) {
      accessOverride = a;
      publishN();
    },
    notificationOpens,
    clipboardWrites,
    failNextClipboardCopy(m) {
      failCopy = m;
    },
    screenshotCalls,
    addShot: (o) => addShot(o?.w ?? 640, o?.h ?? 400).id,
    failNextScreenshot(m) {
      failShot = m;
    },
    setSavePath(p) {
      savePath = p;
    },
    resetShots() {
      saveShots({ next: 1, items: [] });
    },
    clipboardCopyExternal(c) {
      const cfg = settings.clipboard;
      if (!cfg.enabled || cfg.paused) return;
      if (c.kind === 'text') {
        if (!c.text.trim()) return;
        const looksSecret = cfg.skipSensitive && /^(ghp_|sk-)[A-Za-z0-9]{20,}$/.test(c.text.trim());
        if (looksSecret) {
          skippedSensitive++;
          publishClip();
          return;
        }
        const lines = c.text.split('\n').filter((l) => l.trim());
        addClip({
          kind: 'text',
          text: c.text,
          hash: `t:${c.text}`,
          preview: lines.slice(0, 4).join('\n').slice(0, 240),
          chars: [...c.text].length,
          lines: lines.length,
          width: 0,
          height: 0,
          bytes: 0,
          thumb: null,
        });
      } else {
        if (!cfg.images) return;
        if (c.sensitive) {
          skippedSensitive++;
          publishClip();
          return;
        }
        const seed = c.seed ?? 1;
        const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="hsl(${(seed * 67) % 360},70%,50%)"/></svg>`;
        addClip({ kind: 'image', text: '', hash: `i:${seed}`, preview: '', chars: 0, lines: 0, width: 1280, height: 720, bytes: 4096, thumb: `data:image/svg+xml;base64,${btoa(svg)}` });
      }
    },
  };
  window.__harness = harness;

  async function applyPatch(patch: SettingsPatch) {
    // Same semantics as the Rust backend: `null` removes a key, then missing keys fall back to defaults.
    settings = mergePatch(structuredClone(defaults) as Settings, mergePatch(settings, patch));
    try {
      localStorage.setItem(KEY, JSON.stringify(settings));
    } catch {
      /* ignore */
    }
    settingsSubs.forEach((cb) => cb(settings));
    reconfigureN();
    reconfigureClip();
    return { settings, warnings: [] as string[] };
  }
  harness.patchSettings = async (p) => void (await applyPatch(p));

  return {
    kind: 'test',
    getSettings: async () => settings,
    updateSettings: (patch: SettingsPatch) => applyPatch(patch),
    onSettingsChanged: (cb) => sub(settingsSubs, cb),
    listMonitors: async () => [
      { index: 0, name: 'Test display 1', x: 0, y: 0, width: 1920, height: 1080, scale: 1, primary: true },
      { index: 1, name: 'Test display 2', x: 1920, y: 0, width: 2560, height: 1440, scale: 1.25, primary: false },
    ],
    setIslandSize: async (w, h) => void sizes.push([w, h]),
    openSettings: async () => void opened++,
    quit: async () => {},
    onIslandToggle: (cb) => sub(toggleSubs, cb),

    getMedia: async () => ({ session }),
    onMedia: (cb) => sub(mediaSubs, cb),
    async mediaTransport(cmd) {
      calls.push(cmd);
      if (!session) throw new Error('No media session');
      const pos = nowPos(session);
      if (cmd.type === 'seek') session = { ...session, positionMs: cmd.positionMs, positionUpdatedAt: Date.now() };
      else if (cmd.type === 'pause') session = { ...session, status: 'paused', positionMs: pos, positionUpdatedAt: Date.now() };
      else if (cmd.type === 'play') session = { ...session, status: 'playing', positionMs: pos, positionUpdatedAt: Date.now() };
      else if (cmd.type === 'toggle')
        session = { ...session, status: session.status === 'playing' ? 'paused' : 'playing', positionMs: pos, positionUpdatedAt: Date.now() };
      emitMedia();
    },

    async lyricsFetch(track) {
      lyricsCalls.push(track);
      if (!settings.music.lyricsOnline) return null;
      if (lyricsReply && 'error' in lyricsReply) throw new Error(lyricsReply.error);
      return lyricsReply;
    },

    async aiSend({ conversationId, text, context }) {
      const provider = settings.ai.provider;
      const model = settings.ai.models[provider] ?? '';
      aiSends.push({ conversationId, text, context, provider, model });
      if (activeCancel) throw new Error('VEX is still answering. Wait for it to finish or press Stop.');
      const needsKey = ['openai', 'gemini', 'claude', 'openrouter'].includes(provider);
      if (needsKey && !keys.has(provider)) throw new Error(`Add your ${provider} API key in Settings → Assistant.`);
      if (!model) throw new Error('Choose a model in Settings → Assistant.');
      const chats = loadChats();
      let conv = chats.find((c) => c.id === conversationId);
      if (!conv) {
        conv = { id: `c${Date.now().toString(16)}-${chats.length + 1}`, title: 'New chat', createdMs: Date.now(), updatedMs: Date.now(), messages: [] };
        chats.push(conv);
      }
      const user: ChatMessage = { role: 'user', content: text, tsMs: Date.now(), context: context?.text ?? null, contextLabel: context?.label ?? null };
      if (conv.messages.length === 0) conv.title = text.length > 48 ? text.slice(0, 48) + '…' : text;
      conv.messages.push(user);
      conv.updatedMs = Date.now();
      saveChats(chats);

      const requestId = nextRequest++;
      const flag = { cancelled: false };
      activeCancel = flag;
      const convId = conv.id;
      const sc = script;
      void (async () => {
        let reply = '';
        const finish = (kind: AiEvent['kind'], extra: Partial<AiEvent> = {}) => {
          const cur = loadChats();
          const c = cur.find((x) => x.id === convId);
          if (c && reply) {
            c.messages.push({ role: 'assistant', content: reply, tsMs: Date.now(), partial: kind !== 'done' });
            c.updatedMs = Date.now();
            saveChats(cur);
          }
          activeCancel = null;
          emitAi({ requestId, conversationId: convId, kind, ...extra });
        };
        if (sc.silentMs) await new Promise((r) => setTimeout(r, sc.silentMs));
        if (flag.cancelled) return finish('cancelled');
        if (sc.thinkingMs) {
          const end = Date.now() + sc.thinkingMs;
          let chars = 0;
          while (Date.now() < end && !flag.cancelled) {
            await new Promise((r) => setTimeout(r, 40));
            chars += 120;
            emitAi({ requestId, conversationId: convId, kind: 'thinking', thinkingChars: chars });
          }
        }
        for (const chunk of sc.chunks ?? []) {
          await new Promise((r) => setTimeout(r, sc.delayMs ?? 30));
          if (flag.cancelled) return finish('cancelled');
          reply += chunk;
          emitAi({ requestId, conversationId: convId, kind: 'delta', text: chunk });
        }
        if (flag.cancelled) return finish('cancelled');
        if (sc.error) return finish('error', { message: sc.error.message, errorKind: sc.error.kind ?? 'provider' });
        finish('done');
      })();
      return { conversationId: convId, requestId };
    },
    async aiCancel() {
      if (activeCancel) activeCancel.cancelled = true;
    },
    onAiEvent: (cb) => sub(aiSubs, cb),
    aiMetrics: async () => metrics,
    aiMetricsClear: async () => {
      metrics = { requests: [], requestsSent: metrics.requestsSent };
    },
    async aiPreload(model) {
      preloadCalls.push(model);
      return benchRows.find((r) => r.mode === 'preload' && r.model === model) ?? { model, think: false, mode: 'preload', ok: true, error: null, wallMs: 1200, headersMs: 1200, firstThinkingMs: null, firstTokenMs: null, visibleChars: 0, thinkingChars: 0, payloadBytes: 80, usage: null };
    },
    async aiBenchmark(models, includeThinking) {
      benchCalls.push({ models, includeThinking });
      const rows = benchRows.filter((r) => models.includes(r.model));
      for (const r of rows) {
        await new Promise((res) => setTimeout(res, 60));
        benchSubs.forEach((cb) => cb(r));
      }
      return rows;
    },
    onBenchRow: (cb) => sub(benchSubs, cb),
    aiKeyStatus: async (p) => keys.has(p),
    async aiSetKey(p, k) {
      if (!k.trim() || /\s/.test(k.trim())) throw new Error('API keys can\'t contain spaces or line breaks. Paste just the key.');
      if (p === 'ollama') throw new Error("Ollama doesn't use an API key.");
      keys.set(p, k.trim());
    },
    aiClearKey: async (p) => void keys.delete(p),
    async aiListModels(p) {
      if (p === 'ollama') {
        if (!ollama.running) throw new Error("Can't reach Ollama at http://localhost:11434. Make sure Ollama is running.");
        return ollama.models;
      }
      if (!keys.has(p)) throw new Error(`Add your ${p} API key in Settings → Assistant.`);
      if (!validKeys.includes(keys.get(p)!)) throw new Error(`${p} rejected the API key. Check or replace it in Settings → Assistant.`);
      return [`${p}-model-a`, `${p}-model-b`];
    },
    async aiTest(p): Promise<AiTestResult> {
      if (p === 'ollama') {
        if (!ollama.running) return { ok: false, local: true, models: [], message: "Can't reach Ollama at http://localhost:11434. Make sure Ollama is running (open it from the Start menu, or run `ollama serve`)." };
        if (ollama.models.length === 0)
          return { ok: true, local: true, models: [], message: 'Ollama is running at http://localhost:11434, but no models are installed yet. Install one yourself, for example run `ollama pull llama3.2` in a terminal. VEX never downloads models for you.' };
        return { ok: true, local: true, models: ollama.models, message: `Ollama is running at http://localhost:11434 with ${ollama.models.length} installed model(s).` };
      }
      if (!keys.has(p)) return { ok: false, local: false, models: [], message: `Add your ${p} API key in Settings → Assistant.` };
      if (!validKeys.includes(keys.get(p)!)) return { ok: false, local: false, models: [], message: `${p} rejected the API key. Check or replace it in Settings → Assistant.` };
      return { ok: true, local: false, models: [`${p}-model-a`, `${p}-model-b`], message: `Connected to ${p}. Your API key was accepted (2 models available).` };
    },
    async chatList(): Promise<ConversationSummary[]> {
      return loadChats()
        .map((c) => ({ id: c.id, title: c.title, updatedMs: c.updatedMs, messageCount: c.messages.length }))
        .sort((a, b) => b.updatedMs - a.updatedMs);
    },
    chatGet: async (id) => loadChats().find((c) => c.id === id) ?? null,
    chatDelete: async (id) => saveChats(loadChats().filter((c) => c.id !== id)),
    chatClearAll: async () => saveChats([]),

    getNotifications: async () => nview,
    onNotifications: (cb) => sub(notifSubs, cb),
    async notificationDismiss(id) {
      nview = { ...nview, entries: nview.entries.map((e) => (e.id === id ? { ...e, unread: false } : e)) };
      publishN();
    },
    async notificationDismissAll() {
      nview = { ...nview, entries: nview.entries.map((e) => ({ ...e, unread: false })) };
      publishN();
    },
    async notificationClear(id) {
      nview = { ...nview, entries: nview.entries.filter((e) => e.id !== id) };
      publishN();
    },
    async notificationClearAll() {
      nview = { ...nview, entries: [] };
      publishN();
    },
    async notificationOpen(id) {
      if (!nview.entries.some((e) => e.id === id)) throw new Error('That notification is no longer in the history.');
      notificationOpens.push(id);
      nview = { ...nview, entries: nview.entries.map((e) => (e.id === id ? { ...e, unread: false } : e)) };
      publishN();
    },

    clipboardStatus: async () => clipStatus(),
    onClipboard: (cb) => sub(clipSubs, cb),
    async clipboardList(query) {
      const q = query.trim().toLowerCase();
      const list = [...clipItems]
        .reverse()
        .filter((i) => (i.kind === 'text' ? !q || i.text.toLowerCase().includes(q) : !q))
        .sort((a, b) => Number(b.pinned) - Number(a.pinned));
      return list.map(({ text: _t, hash: _h, ...view }) => view);
    },
    async clipboardCopy(id) {
      if (failCopy) {
        const m = failCopy;
        failCopy = null;
        throw new Error(m);
      }
      const it = clipItems.find((i) => i.id === id);
      if (!it) throw new Error('That entry is no longer in the history.');
      clipboardWrites.push({ kind: it.kind, text: it.kind === 'text' ? it.text : undefined, id });
      clipItems = [...clipItems.filter((i) => i.id !== id), { ...it, tsMs: Date.now() }];
      publishClip();
    },
    async clipboardPin(id, pinned) {
      if (!clipItems.some((i) => i.id === id)) throw new Error('That entry is no longer in the history.');
      clipItems = clipItems.map((i) => (i.id === id ? { ...i, pinned } : i));
      publishClip();
    },
    async clipboardDelete(id) {
      clipItems = clipItems.filter((i) => i.id !== id);
      publishClip();
    },
    async clipboardClear(keepPinned) {
      clipItems = keepPinned ? clipItems.filter((i) => i.pinned) : [];
      publishClip();
    },

    async screenshotCapture(mode: CaptureMode) {
      screenshotCalls.push({ type: 'capture', mode });
      shotFail();
      if (mode === 'screen') addShot(960, 540);
      else if (mode === 'window') addShot(640, 400);
      // region: the real app opens the selection overlays; the capture arrives after the drag
    },
    async screenshotRegionSelect(monitor, x0, y0, x1, y1) {
      screenshotCalls.push({ type: 'regionSelect', monitor, x0, y0, x1, y1, dpr: window.devicePixelRatio });
      shotFail();
      const w = Math.round(Math.abs(x1 - x0) * window.devicePixelRatio);
      const h = Math.round(Math.abs(y1 - y0) * window.devicePixelRatio);
      if (w < 4 || h < 4) throw new Error('The selection was too small.');
      addShot(w, h);
    },
    async screenshotRegionCancel() {
      screenshotCalls.push({ type: 'regionCancel' });
    },
    async screenshotList() {
      return loadShots().items.map((i) => i.view).reverse();
    },
    async screenshotImage(id) {
      const it = loadShots().items.find((i) => i.view.id === id);
      if (!it) throw new Error('That screenshot is no longer available.');
      return it.data;
    },
    async screenshotDelete(id) {
      const st = loadShots();
      saveShots({ ...st, items: st.items.filter((i) => i.view.id !== id) });
      screenshotCalls.push({ type: 'delete', id });
    },
    async screenshotClear() {
      const st = loadShots();
      saveShots({ ...st, items: [] });
      screenshotCalls.push({ type: 'clear' });
    },
    async screenshotCopy(source: ShotSource) {
      screenshotCalls.push({ type: 'copy', id: source.id, bytes: source.data?.length ?? 0, dataUrl: source.data?.slice(0, 22) });
      shotFail();
    },
    async screenshotSave(source: ShotSource, ask: boolean) {
      screenshotCalls.push({ type: 'save', id: source.id, bytes: source.data?.length ?? 0, ask, data: source.data });
      shotFail();
      return savePath;
    },
    async screenshotPickFolder() {
      screenshotCalls.push({ type: 'pickFolder' });
      return 'D:\\Captures';
    },
    async screenshotReveal(path: string) {
      screenshotCalls.push({ type: 'reveal', path });
    },
    async screenshotOpenEditor(id: number) {
      screenshotCalls.push({ type: 'openEditor', id });
      shotChannel.postMessage({ type: 'editor-load', id });
    },
    onScreenshotCaptured: (cb) => sub(shotCapturedSubs, cb),
    onScreenshotError: (cb) => sub(shotErrorSubs, cb),
    onEditorLoad: (cb) => sub(editorLoadSubs, cb),

    // Tauri events reach every window; BroadcastChannel stands in for that across test pages.
    assistantPreview: async (s) => {
      assistantSubs.forEach((cb) => cb(s));
      channel.postMessage(s);
    },
    onAssistantState: (cb) => sub(assistantSubs, cb),
  };
}
