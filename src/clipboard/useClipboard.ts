import { useCallback, useEffect, useRef, useState } from 'react';
import { useBackend, useSubscription } from '../backend/hooks';
import type { ClipboardItem, ClipboardStatus } from '../shared/types';

const OFF: ClipboardStatus = { enabled: false, paused: false, recording: false, count: 0, pinned: 0, imageBytes: 0, skippedSensitive: 0, note: null, persist: false };

/** Clipboard status always; the item list only while `listen` is true (the panel is showing). */
export function useClipboard(listen: boolean, query = '') {
  const backend = useBackend();
  const [status, setStatus] = useState<ClipboardStatus>(OFF);
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const seq = useRef(0);

  useEffect(() => {
    let alive = true;
    backend.clipboardStatus().then((s) => alive && setStatus(s)).catch(() => {});
    return () => {
      alive = false;
    };
  }, [backend]);
  useSubscription<ClipboardStatus>((cb) => backend.onClipboard(cb), setStatus);

  const reload = useCallback(() => {
    const mine = ++seq.current;
    backend
      .clipboardList(query)
      .then((l) => mine === seq.current && setItems(l))
      .catch(() => mine === seq.current && setItems([]));
  }, [backend, query]);

  // Reload when shown, when the query changes, and whenever the history changed (count, pins, notes).
  useEffect(() => {
    if (listen) reload();
    else setItems([]); // nothing is kept in the page while the panel is closed
  }, [listen, reload, status]);

  return { status, items };
}
