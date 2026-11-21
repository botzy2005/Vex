import { useCallback, useEffect, useRef, useState } from 'react';
import { useBackend, useSubscription } from '../backend/hooks';
import type { HubLayout, HubStatusEntry, HubTimer, WidgetDef } from '../shared/types';
import { remainingMs } from './timeFormat';

/**
 * The saved dashboard plus the catalog. `apply` runs a pure layout operation, shows the result at once
 * and saves it; if the native side kept something different (it validates), that wins. Saves are
 * sent one after another so an older answer can never overwrite a newer edit.
 */
export function useHubLayout() {
  const backend = useBackend();
  const [catalog, setCatalog] = useState<WidgetDef[]>([]);
  const [layout, setLayout] = useState<HubLayout | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const latest = useRef<HubLayout | null>(null);
  const chain = useRef<Promise<void>>(Promise.resolve());
  const pending = useRef(0);

  useEffect(() => {
    let live = true;
    void Promise.all([backend.hubCatalog(), backend.hubLayout()]).then(([c, l]) => {
      if (!live) return;
      latest.current = l;
      setCatalog(c);
      setLayout(l);
    });
    return () => {
      live = false;
    };
  }, [backend]);

  useSubscription<HubLayout>(
    (cb) => backend.onHubChanged(cb),
    (l) => {
      if (pending.current > 0) return; // our own save is in flight; its answer is the truth
      latest.current = l;
      setLayout(l);
    },
  );

  const apply = useCallback(
    (op: (l: HubLayout) => HubLayout) => {
      const cur = latest.current;
      if (!cur) return;
      const next = op(cur);
      if (next === cur) return;
      latest.current = next;
      setLayout(next);
      pending.current += 1;
      chain.current = chain.current.then(async () => {
        try {
          const kept = await backend.hubSaveLayout(latest.current ?? next);
          setSaveError(null);
          if (pending.current === 1) {
            latest.current = kept;
            setLayout(kept);
          }
        } catch (e) {
          setSaveError(String(e));
        } finally {
          pending.current -= 1;
        }
      });
    },
    [backend],
  );

  return { catalog, layout, apply, saveError };
}

/** Statuses widgets asked the island to show (empty when the Hub is closed). */
export function useHubStatuses(): HubStatusEntry[] {
  const backend = useBackend();
  const [list, setList] = useState<HubStatusEntry[]>([]);
  useEffect(() => {
    let live = true;
    void backend.hubStatuses().then((s) => live && setList(s));
    return () => {
      live = false;
    };
  }, [backend]);
  useSubscription<HubStatusEntry[]>((cb) => backend.onHubStatus(cb), setList);
  return list;
}

/**
 * Shared timers. The list comes from the native side (which owns the clock); `now` re-renders once a
 * second, and only while a timer is running and `active` is true, so an idle consumer costs nothing.
 */
export function useTimers(active = true): { timers: HubTimer[]; now: number } {
  const backend = useBackend();
  const [timers, setTimers] = useState<HubTimer[]>([]);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    let live = true;
    void backend.timersList().then((t) => live && setTimers(t));
    return () => {
      live = false;
    };
  }, [backend]);
  useSubscription<HubTimer[]>((cb) => backend.onTimers(cb), setTimers);
  const running = timers.some((t) => t.state === 'running');
  useEffect(() => {
    setNow(Date.now());
    if (!active || !running) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [active, running]);
  return { timers, now };
}

export { remainingMs };

/** True while the page is visible (the Hub window is not minimised or hidden). */
export function usePageActive(): boolean {
  const [active, setActive] = useState(() => document.visibilityState !== 'hidden');
  useEffect(() => {
    const on = () => setActive(document.visibilityState !== 'hidden');
    document.addEventListener('visibilitychange', on);
    return () => document.removeEventListener('visibilitychange', on);
  }, []);
  return active;
}
