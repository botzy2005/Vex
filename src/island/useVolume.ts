import { useCallback, useEffect, useRef, useState } from 'react';
import type { Backend } from '../backend/backend';
import type { VolumeState } from '../shared/types';
import { SLIDER_INTERVAL_MS } from './volume';

/**
 * The Windows volume for the island. Nothing is requested while `enabled` is false (the speaker button is
 * off, or the island is collapsed); once enabled it loads the level once and then only reacts to pushed events.
 * While the slider is being dragged, incoming echoes of our own changes are ignored so the thumb does not fight
 * the pointer, and slider movement is sent at most about 30 times a second (the last value always gets through).
 */
export function useVolume(backend: Backend, enabled: boolean) {
  const [state, setState] = useState<VolumeState | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const draggingRef = useRef(false);
  const pending = useRef<number | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const lastSent = useRef(0);

  useEffect(() => {
    if (!enabled) return;
    let dead = false;
    let off: (() => void) | undefined;
    backend.volumeGet().then((s) => !dead && setState(s)).catch((e) => !dead && setFailure(String(e)));
    backend
      .onVolume((s) => {
        if (!draggingRef.current) setState(s);
      })
      .then((u) => (dead ? u() : (off = u)));
    return () => {
      dead = true;
      off?.();
    };
  }, [backend, enabled]);

  const apply = useCallback(
    (p: Promise<VolumeState>) =>
      p
        .then((s) => {
          setFailure(null);
          if (!draggingRef.current) setState(s);
        })
        .catch((e) => setFailure(String(e))),
    [],
  );

  const flush = useCallback(() => {
    timer.current = undefined;
    const v = pending.current;
    if (v === null) return;
    pending.current = null;
    lastSent.current = Date.now();
    void apply(backend.volumeSet(v));
  }, [apply, backend]);

  /** Slider movement: show it at once, send it rate-limited. */
  const slide = useCallback(
    (percent: number) => {
      draggingRef.current = true;
      setState((s) => (s ? { ...s, percent, muted: percent > 0 ? false : s.muted } : s));
      pending.current = percent;
      if (timer.current !== undefined) return;
      const wait = Math.max(0, SLIDER_INTERVAL_MS - (Date.now() - lastSent.current));
      timer.current = window.setTimeout(flush, wait);
    },
    [flush],
  );

  /** Pointer released: send the final value and go back to following the system. */
  const slideEnd = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
    if (pending.current !== null) {
      const v = pending.current;
      pending.current = null;
      void backend.volumeSet(v).then((s) => setState(s)).catch((e) => setFailure(String(e))).finally(() => (draggingRef.current = false));
    } else draggingRef.current = false;
  }, [backend]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return {
    state,
    failure,
    slide,
    slideEnd,
    step: (d: 1 | -1) => void apply(backend.volumeStep(d)),
    toggleMute: () => void apply(backend.volumeToggleMute()),
  };
}
