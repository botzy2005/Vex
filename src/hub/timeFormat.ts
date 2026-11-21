import type { HubTimer } from '../shared/types';

/** Milliseconds left on a timer at `now` (the same rule as the native engine). */
export function remainingMs(t: HubTimer, now: number): number {
  switch (t.state) {
    case 'idle':
      return t.durationMs;
    case 'running':
      return Math.max(0, t.endsAt - now);
    case 'paused':
      return t.remainingMs;
    case 'finished':
      return 0;
  }
}

/** 59 s -> "0:59", 25 min -> "25:00", 90 min -> "1:30:00", 3 days -> "3d 0h". Rounds up so it never shows 0:00 early. */
export function formatRemaining(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const d = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (d >= 1) return `${d}d ${h}h`;
  const mm = String(m).padStart(2, '0');
  const ss = String(s).padStart(2, '0');
  return h >= 1 ? `${h}:${mm}:${ss}` : `${m}:${ss}`;
}
