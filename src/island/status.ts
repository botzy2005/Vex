import type { HubStatusEntry } from '../shared/types';

/** The status line worth showing on the collapsed island: highest priority first (ambient 0 stays in the Hub). */
export function pickStatus(list: HubStatusEntry[], enabled: boolean): HubStatusEntry | null {
  if (!enabled) return null;
  let best: HubStatusEntry | null = null;
  for (const s of list) if (s.priority >= 1 && (!best || s.priority > best.priority)) best = s;
  return best;
}

export function statusText(s: HubStatusEntry): string {
  return s.detail ? `${s.label} · ${s.detail}` : s.label;
}
