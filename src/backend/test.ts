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
  /** Simulated Ollama service: whether it runs and which models it has installed. */
  setOllama(o: { running: boolean; models: string[] }): void;
  /** Keys the simulated provider accepts (anything else is rejected as invalid). */
  setValidKeys(keys: string[]): void;
  readonly aiSends: Array<{ conversationId: string | null; text: string; context: ContextPayload | null; provider: string; model: string }>;
  /** Forget everything in memory, as if the app restarted (history stays in storage). */
  restartAi(): void;
  readonly settingsOpened: number;
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
    aiSends,
    restartAi: () => {
      activeCancel = null;
    },
    get settingsOpened() {
      return opened;
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

    // Tauri events reach every window; BroadcastChannel stands in for that across test pages.
    assistantPreview: async (s) => {
      assistantSubs.forEach((cb) => cb(s));
      channel.postMessage(s);
    },
    onAssistantState: (cb) => sub(assistantSubs, cb),
  };
}
