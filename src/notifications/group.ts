import type { NotificationApp, NotificationEntry } from '../shared/types';

export interface NotificationGroup {
  key: string;
  appId: string;
  appName: string;
  icon: string | null;
  /** Newest first. */
  items: NotificationEntry[];
  unread: number;
  latestMs: number;
}

/**
 * Group history by application (newest group first, newest item first inside it).
 * With grouping off every notification is its own group, in plain time order.
 */
export function groupEntries(entries: NotificationEntry[], grouping: boolean, apps: NotificationApp[]): NotificationGroup[] {
  const icons = new Map(apps.map((a) => [a.id, a.icon]));
  const sorted = [...entries].sort((a, b) => b.tsMs - a.tsMs || b.id - a.id);
  const make = (key: string, first: NotificationEntry): NotificationGroup => ({
    key,
    appId: first.appId,
    appName: first.appName,
    icon: icons.get(first.appId) ?? null,
    items: [],
    unread: 0,
    latestMs: first.tsMs,
  });
  if (!grouping) {
    return sorted.map((e) => {
      const g = make(`n${e.id}`, e);
      g.items.push(e);
      g.unread = e.unread ? 1 : 0;
      return g;
    });
  }
  const groups = new Map<string, NotificationGroup>();
  for (const e of sorted) {
    let g = groups.get(e.appId);
    if (!g) groups.set(e.appId, (g = make(`a${e.appId}`, e)));
    g.items.push(e);
    if (e.unread) g.unread += 1;
  }
  return [...groups.values()]; // insertion order = newest group first, because `sorted` is newest first
}

/** "now", "5 min ago", "2 h ago", "3 d ago". */
export function relativeTime(tsMs: number, nowMs: number): string {
  const s = Math.max(0, Math.round((nowMs - tsMs) / 1000));
  if (s < 45) return 'now';
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  return `${Math.round(h / 24)} d ago`;
}

export interface PreviewState {
  /** Highest entry id already accounted for. */
  maxId: number;
  /** The entry to flash in the collapsed island, if one just arrived. */
  preview: NotificationEntry | null;
}

/**
 * Decide whether a new view contains a notification to flash. The first view only sets the baseline
 * (so reopening VEX never replays history); later views preview the newest unread entry that is new.
 */
export function nextPreview(prev: PreviewState | null, entries: NotificationEntry[]): PreviewState {
  const maxId = entries.reduce((m, e) => Math.max(m, e.id), 0);
  if (!prev) return { maxId, preview: null };
  const fresh = entries.filter((e) => e.id > prev.maxId && e.unread);
  const preview = fresh.length ? fresh.reduce((a, b) => (b.id > a.id ? b : a)) : null;
  return { maxId: Math.max(maxId, prev.maxId), preview };
}

/** Plain words for each state the module can be in (shown in the panel and in Settings). */
export function accessMessage(access: { state: string; reason?: string }, enabled: boolean): string | null {
  if (!enabled) return 'Notifications are off. Turn them on in Settings → Notifications.';
  switch (access.state) {
    case 'allowed':
      return null;
    case 'denied':
      return 'Windows has not given VEX permission to read notifications. Allow it in Windows Settings → Privacy & security → Notifications.';
    case 'unavailable':
      return access.reason || 'Windows notifications are not available.';
    default:
      return 'Waiting for Windows…';
  }
}
