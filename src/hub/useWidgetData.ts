import { useCallback, useEffect, useRef, useState } from 'react';
import { useBackend } from '../backend/hooks';

const SAVE_DELAY_MS = 400;

/**
 * A widget's persisted document. Loads once, saves a moment after the last change, and saves any
 * unsaved change when the widget unmounts or the window closes. Shape checking is the widget's job:
 * pass `normalize` to repair or reject whatever was stored (older versions, hand edits).
 */
export function useWidgetData<T>(kind: string, initial: T, normalize?: (raw: unknown) => T) {
  const backend = useBackend();
  const [value, setValue] = useState<T>(initial);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(value);
  const dirty = useRef(false);
  const timer = useRef<number | undefined>(undefined);

  const flush = useCallback(() => {
    window.clearTimeout(timer.current);
    if (!dirty.current) return;
    dirty.current = false;
    backend.hubDataSet(kind, latest.current).then(
      () => setError(null),
      (e) => setError(String(e)),
    );
  }, [backend, kind]);

  useEffect(() => {
    let live = true;
    backend.hubDataGet(kind).then(
      (raw) => {
        if (!live) return;
        const next = raw == null ? initial : normalize ? normalize(raw) : (raw as T);
        latest.current = next;
        setValue(next);
        setLoaded(true);
      },
      (e) => {
        if (!live) return;
        setError(String(e));
        setLoaded(true);
      },
    );
    return () => {
      live = false;
    };
    // `initial` and `normalize` are read once on purpose.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend, kind]);

  useEffect(() => {
    window.addEventListener('pagehide', flush);
    return () => {
      window.removeEventListener('pagehide', flush);
      flush();
    };
  }, [flush]);

  const update = useCallback(
    (next: T | ((prev: T) => T)) => {
      const v = typeof next === 'function' ? (next as (p: T) => T)(latest.current) : next;
      latest.current = v;
      dirty.current = true;
      setValue(v);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(flush, SAVE_DELAY_MS);
    },
    [flush],
  );

  return { value, update, loaded, error, flush };
}
