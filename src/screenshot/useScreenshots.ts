import { useCallback, useEffect, useRef, useState } from 'react';
import { useBackend, useSubscription } from '../backend/hooks';
import type { Settings, ShotView } from '../shared/types';
import { playShutter } from './sound';

export type ShotToast = { kind: 'shot'; shot: ShotView } | { kind: 'error'; message: string };

export const TOAST_MS = 7000;
const ERROR_MS = 6000;

/** The confirmation the island shows after a capture (and for a failed one). Sound is played here, once per capture. */
export function useScreenshotToast(settings: Settings['screenshot'] | undefined) {
  const backend = useBackend();
  const [toast, setToast] = useState<ShotToast | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const soundOn = useRef(false);
  soundOn.current = !!settings?.sound;

  const arm = useCallback((ms: number) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setToast(null), ms);
  }, []);

  useSubscription<ShotView>((cb) => backend.onScreenshotCaptured(cb), (shot) => {
    setToast({ kind: 'shot', shot });
    arm(TOAST_MS);
    if (soundOn.current) playShutter();
  });
  useSubscription<string>((cb) => backend.onScreenshotError(cb), (message) => {
    setToast({ kind: 'error', message });
    arm(ERROR_MS);
  });
  useEffect(() => () => window.clearTimeout(timer.current), []);

  return {
    toast,
    dismiss: () => {
      window.clearTimeout(timer.current);
      setToast(null);
    },
    /** Keep the toast up a while longer (the user is using it). */
    extend: () => arm(TOAST_MS),
    fail: (message: string) => {
      setToast({ kind: 'error', message });
      arm(ERROR_MS);
    },
  };
}

/** The capture history, reloaded when shown and whenever a capture arrives. */
export function useShotList(active: boolean) {
  const backend = useBackend();
  const [items, setItems] = useState<ShotView[]>([]);
  const seq = useRef(0);
  const reload = useCallback(() => {
    const mine = ++seq.current;
    backend.screenshotList().then((l) => mine === seq.current && setItems(l)).catch(() => mine === seq.current && setItems([]));
  }, [backend]);
  useEffect(() => {
    if (active) reload();
    else setItems([]); // thumbnails are not kept in the page while the panel is closed
  }, [active, reload]);
  useSubscription<ShotView>((cb) => backend.onScreenshotCaptured(cb), () => active && reload());
  return { items, reload };
}
