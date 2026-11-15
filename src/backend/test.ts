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
  AssistantState,
  MediaSession,
  MediaState,
  Settings,
  SettingsPatch,
  TransportCommand,
} from '../shared/types';
import type { Backend, Unsub } from './backend';

const KEY = 'islet-test-settings';

export interface Harness {
  /** Replace (or clear) the simulated external media session. */
  setSession(s: Partial<MediaSession> | null): void;
  emitAssistant(state: AssistantState): void;
  emitIslandToggle(): void;
  readonly transportCalls: TransportCommand[];
  readonly windowSizes: Array<[number, number]>;
  readonly assistantSends: string[];
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
  const sends: string[] = [];
  let opened = 0;

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

  window.__harness = {
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
    emitAssistant: (s) => assistantSubs.forEach((cb) => cb(s)),
    emitIslandToggle: () => toggleSubs.forEach((cb) => cb()),
    transportCalls: calls,
    windowSizes: sizes,
    assistantSends: sends,
    get settingsOpened() {
      return opened;
    },
  };

  return {
    kind: 'test',
    getSettings: async () => settings,
    async updateSettings(patch: SettingsPatch) {
      // Same semantics as the Rust backend: `null` removes a key, then missing keys fall back to defaults.
      settings = mergePatch(structuredClone(defaults) as Settings, mergePatch(settings, patch));
      try {
        localStorage.setItem(KEY, JSON.stringify(settings));
      } catch {
        /* ignore */
      }
      settingsSubs.forEach((cb) => cb(settings));
      return { settings, warnings: [] };
    },
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

    async assistantSend(text) {
      sends.push(text);
      // Mirrors the real shell until Milestone 5: no provider is connected.
      throw new Error('No AI provider is connected yet (Milestone 5).');
    },
    // Tauri events reach every window; BroadcastChannel stands in for that across test pages.
    assistantPreview: async (s) => {
      assistantSubs.forEach((cb) => cb(s));
      channel.postMessage(s);
    },
    onAssistantState: (cb) => sub(assistantSubs, cb),
  };
}
