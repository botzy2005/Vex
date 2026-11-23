/** Countdown timers rules. Pure, so duration parsing, limits and the island line are unit-tested. */
import type { HubTimer } from '../../../shared/types';

export const OWNER = 'countdown';
export const MAX_TIMERS = 8;
export const MAX_MS = (99 * 3600 + 59 * 60 + 59) * 1000;
export const PRESETS: Array<{ label: string; ms: number }> = [
  { label: '1 min', ms: 60_000 },
  { label: '5 min', ms: 5 * 60_000 },
  { label: '10 min', ms: 10 * 60_000 },
  { label: '15 min', ms: 15 * 60_000 },
  { label: '30 min', ms: 30 * 60_000 },
  { label: '1 h', ms: 3600_000 },
];

export interface CountdownDoc {
  version: 1;
  /** Play a chime when a countdown ends. */
  sound: boolean;
}

export const DEFAULT_DOC: CountdownDoc = { version: 1, sound: true };

export function normalize(raw: unknown): CountdownDoc {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  return { version: 1, sound: typeof r.sound === 'boolean' ? r.sound : true };
}

const field = (v: string): number => {
  const n = Number(v.trim() === '' ? 0 : v);
  return Number.isFinite(n) && n >= 0 ? Math.floor(n) : NaN;
};

/** Hours, minutes and seconds typed by the user to milliseconds; a message when it cannot be used. */
export function parseDuration(h: string, m: string, s: string): { ok: true; ms: number } | { ok: false; reason: string } {
  const hh = field(h);
  const mm = field(m);
  const ss = field(s);
  if ([hh, mm, ss].some(Number.isNaN)) return { ok: false, reason: 'Use whole numbers for hours, minutes and seconds.' };
  const ms = ((hh * 60 + mm) * 60 + ss) * 1000;
  if (ms < 1000) return { ok: false, reason: 'Set a time of at least one second.' };
  if (ms > MAX_MS) return { ok: false, reason: 'The longest countdown is 99 hours 59 minutes 59 seconds.' };
  return { ok: true, ms };
}

export const cleanLabel = (s: string): string => s.trim().replace(/\s+/g, ' ').slice(0, 40);

/** "5 min", "1 h 30 min", "45 s": the length a countdown was set to. */
export function describeLength(ms: number): string {
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const parts = [h ? `${h} h` : '', m ? `${m} min` : '', s ? `${s} s` : ''].filter(Boolean);
  return parts.join(' ') || '0 s';
}

/** This widget's timers, in creation order. */
export const mine = (timers: HubTimer[]): HubTimer[] => timers.filter((t) => t.owner === OWNER).sort((a, b) => a.id - b.id);

/** The running countdown that ends first (what the island shows). */
export function soonest(timers: HubTimer[]): (HubTimer & { state: 'running' }) | null {
  let best: (HubTimer & { state: 'running' }) | null = null;
  for (const t of mine(timers)) if (t.state === 'running' && (!best || t.endsAt < best.endsAt)) best = t as HubTimer & { state: 'running' };
  return best;
}
