import { useEffect, useRef, useState } from 'react';
import { useBackend, useSubscription } from '../backend/hooks';
import type { NotificationEntry, NotificationsView } from '../shared/types';
import { nextPreview, type PreviewState } from './group';

const EMPTY: NotificationsView = { access: { state: 'idle' }, entries: [], apps: [], unread: 0 };
export const PREVIEW_MS = 6000;

/** The notification history, plus the entry (if any) that just arrived and should flash in the island. */
export function useNotifications(withPreview: boolean) {
  const backend = useBackend();
  const [view, setView] = useState<NotificationsView>(EMPTY);
  const [preview, setPreview] = useState<NotificationEntry | null>(null);
  const state = useRef<PreviewState | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const apply = (v: NotificationsView) => {
    setView(v);
    if (!withPreview) return;
    const next = nextPreview(state.current, v.entries);
    state.current = next;
    if (next.preview) {
      setPreview(next.preview);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setPreview(null), PREVIEW_MS);
    }
    // A dismissed or cleared entry must not keep flashing.
    setPreview((p) => (p && v.entries.some((e) => e.id === p.id && e.unread) ? p : null));
  };

  useEffect(() => {
    let alive = true;
    backend.getNotifications().then((v) => alive && apply(v ?? EMPTY)).catch(() => {});
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend]);
  useSubscription<NotificationsView>((cb) => backend.onNotifications(cb), apply);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  return {
    view,
    preview,
    hidePreview: () => setPreview(null),
  };
}
