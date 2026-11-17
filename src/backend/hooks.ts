import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import type { Backend, Unsub } from './backend';
import { hasNull, mergeSettings } from '../shared/merge';
import type { AssistantState, MediaState, Settings, SettingsPatch } from '../shared/types';

export const BackendContext = createContext<Backend | null>(null);

export function useBackend(): Backend {
  const b = useContext(BackendContext);
  if (!b) throw new Error('BackendContext missing');
  return b;
}

/** Subscribe to a backend event for the lifetime of the component. */
export function useSubscription<T>(subscribe: (cb: (v: T) => void) => Promise<Unsub>, handler: (v: T) => void) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    let dead = false;
    let off: Unsub | undefined;
    subscribe((v) => ref.current(v)).then((u) => (dead ? u() : (off = u)));
    return () => {
      dead = true;
      off?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

export function useSettings() {
  const backend = useBackend();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);

  useEffect(() => {
    backend.getSettings().then(setSettings);
  }, [backend]);
  useSubscription<Settings>((cb) => backend.onSettingsChanged(cb), setSettings);

  const update = useCallback(
    async (patch: SettingsPatch) => {
      // Optimistic for snappy sliders; `null` (reset-to-default) waits for the backend.
      if (!hasNull(patch)) setSettings((cur) => (cur ? mergeSettings(cur, patch) : cur));
      try {
        const r = await backend.updateSettings(patch);
        setSettings(r.settings);
        setWarnings(r.warnings);
      } catch (e) {
        setWarnings([String(e)]);
        setSettings(await backend.getSettings());
      }
    },
    [backend],
  );

  return { settings, update, warnings };
}

export function useMedia(): MediaState {
  const backend = useBackend();
  const [media, setMedia] = useState<MediaState>({ session: null });
  useEffect(() => {
    backend.getMedia().then(setMedia).catch(() => setMedia({ session: null }));
  }, [backend]);
  useSubscription<MediaState>((cb) => backend.onMedia(cb), setMedia);
  return media;
}

export interface AssistantHook {
  state: AssistantState;
  /** Last status or error text to show the user. */
  message: string | null;
  /** Drive the face (chat does this while it waits for and streams an answer). */
  set(state: AssistantState, message?: string | null): void;
}

const ERROR_RESET_MS = 6000;

export function useAssistant(): AssistantHook {
  const backend = useBackend();
  const [state, setState] = useState<AssistantState>('idle');
  const [message, setMessage] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const set = useCallback((s: AssistantState, msg: string | null = null) => {
    window.clearTimeout(timer.current);
    setState(s);
    setMessage(s === 'error' ? msg : null);
    if (s === 'error') timer.current = window.setTimeout(() => setState('idle'), ERROR_RESET_MS);
  }, []);

  // Also follows states sent from elsewhere (the Settings window previews the face this way).
  useSubscription<AssistantState>((cb) => backend.onAssistantState(cb), (s) => set(s));
  useEffect(() => () => window.clearTimeout(timer.current), []);

  return { state, message, set };
}

/** Resolve the effective colour scheme ('system' follows the OS). */
export function useResolvedTheme(theme: Settings['appearance']['theme'] | undefined): 'dark' | 'light' {
  const [prefersDark, setPrefersDark] = useState(() => window.matchMedia('(prefers-color-scheme: dark)').matches);
  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const on = (e: MediaQueryListEvent) => setPrefersDark(e.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  if (theme === 'light') return 'light';
  if (theme === 'dark' || theme === undefined) return 'dark';
  return prefersDark ? 'dark' : 'light';
}
