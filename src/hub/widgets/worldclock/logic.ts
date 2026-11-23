/** World Clock rules. Pure (the clock is passed in), so time zones, daylight saving and limits are unit-tested. */
import { CITIES, type City } from './cities';

export interface ClockEntry {
  /** IANA zone id. */
  zone: string;
  /** What to show; defaults to the catalog name. */
  label: string;
}

export interface ClockDoc {
  version: 1;
  /** `null` follows the system's 12/24 hour preference. */
  hour12: boolean | null;
  clocks: ClockEntry[];
}

export const MAX_CLOCKS = 12;
export const DEFAULT_DOC: ClockDoc = {
  version: 1,
  hour12: null,
  clocks: [
    { zone: 'Europe/London', label: 'London' },
    { zone: 'America/New_York', label: 'New York' },
    { zone: 'Asia/Tokyo', label: 'Tokyo' },
  ],
};

export function isValidZone(zone: string): boolean {
  if (typeof zone !== 'string' || zone.length === 0 || zone.length > 64) return false;
  try {
    new Intl.DateTimeFormat('en-US', { timeZone: zone });
    return true;
  } catch {
    return false;
  }
}

export const cityOf = (zone: string): City | undefined => CITIES.find((c) => c.zone === zone);

export function normalize(raw: unknown): ClockDoc {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  if (!Array.isArray(r.clocks)) return DEFAULT_DOC;
  const seen = new Set<string>();
  const clocks: ClockEntry[] = [];
  for (const c of r.clocks) {
    if (!c || typeof c !== 'object') continue;
    const o = c as Record<string, unknown>;
    if (typeof o.zone !== 'string' || seen.has(o.zone) || !isValidZone(o.zone)) continue;
    seen.add(o.zone);
    const label = typeof o.label === 'string' && o.label.trim() !== '' ? o.label.trim().slice(0, 40) : (cityOf(o.zone)?.name ?? o.zone);
    clocks.push({ zone: o.zone, label });
    if (clocks.length >= MAX_CLOCKS) break;
  }
  return { version: 1, hour12: typeof r.hour12 === 'boolean' ? r.hour12 : null, clocks };
}

export function addClock(doc: ClockDoc, city: City): ClockDoc {
  if (doc.clocks.length >= MAX_CLOCKS || doc.clocks.some((c) => c.zone === city.zone)) return doc;
  return { ...doc, clocks: [...doc.clocks, { zone: city.zone, label: city.name }] };
}

export function removeClock(doc: ClockDoc, zone: string): ClockDoc {
  return doc.clocks.some((c) => c.zone === zone) ? { ...doc, clocks: doc.clocks.filter((c) => c.zone !== zone) } : doc;
}

export function moveClock(doc: ClockDoc, zone: string, dir: -1 | 1): ClockDoc {
  const i = doc.clocks.findIndex((c) => c.zone === zone);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= doc.clocks.length) return doc;
  const clocks = [...doc.clocks];
  [clocks[i], clocks[j]] = [clocks[j], clocks[i]];
  return { ...doc, clocks };
}

/** Cities matching every word of `query` in name, country or zone id (zones already added are left out). */
export function searchCities(query: string, exclude: string[] = [], limit = 8): City[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return CITIES.filter((c) => !exclude.includes(c.zone) && words.every((w) => `${c.name} ${c.country} ${c.zone.replace(/_/g, ' ')}`.toLowerCase().includes(w))).slice(0, limit);
}

interface Parts {
  y: number; mo: number; d: number; h: number; mi: number; s: number;
}

const partsCache = new Map<string, Intl.DateTimeFormat>();
function partsOf(zone: string, ms: number): Parts {
  let f = partsCache.get(zone);
  if (!f) {
    f = new Intl.DateTimeFormat('en-US', { timeZone: zone, hourCycle: 'h23', year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric', second: 'numeric' });
    partsCache.set(zone, f);
  }
  const o: Record<string, number> = {};
  for (const p of f.formatToParts(new Date(ms))) if (p.type !== 'literal') o[p.type] = Number(p.value);
  return { y: o.year, mo: o.month, d: o.day, h: o.hour % 24, mi: o.minute, s: o.second };
}

/** Minutes the zone is ahead of UTC at that instant (daylight saving included). */
export function offsetMinutes(zone: string, atMs: number): number {
  const p = partsOf(zone, atMs);
  const asUtc = Date.UTC(p.y, p.mo - 1, p.d, p.h, p.mi, p.s);
  return Math.round((asUtc - Math.floor(atMs / 1000) * 1000) / 60000);
}

/** Wall-clock time in the zone, "14:05" or "2:05 pm". A 24-hour display always has two digits ("07:05"). */
export function timeIn(zone: string, atMs: number, hour12: boolean | null): string {
  const o: Intl.DateTimeFormatOptions = { timeZone: zone, hour: 'numeric', minute: '2-digit' };
  if (hour12 !== null) o.hour12 = hour12;
  let f = new Intl.DateTimeFormat([], o);
  if (f.resolvedOptions().hour12 === false) f = new Intl.DateTimeFormat([], { ...o, hour: '2-digit' });
  return f.format(new Date(atMs));
}

/** Hour 0-23 in the zone. */
export const hourIn = (zone: string, atMs: number): number => partsOf(zone, atMs).h;

/** Daylight, roughly: 06:00 to 17:59 local. */
export const isDaytime = (zone: string, atMs: number): boolean => {
  const h = hourIn(zone, atMs);
  return h >= 6 && h < 18;
};

const dayNumber = (p: Parts): number => Math.floor(Date.UTC(p.y, p.mo - 1, p.d) / 86400000);

/** "Today", "Tomorrow", "Yesterday", or a short date, relative to the viewer's own zone. */
export function dayLabel(zone: string, localZone: string, atMs: number): string {
  const delta = dayNumber(partsOf(zone, atMs)) - dayNumber(partsOf(localZone, atMs));
  if (delta === 0) return 'Today';
  if (delta === 1) return 'Tomorrow';
  if (delta === -1) return 'Yesterday';
  return new Intl.DateTimeFormat([], { timeZone: zone, weekday: 'short', day: 'numeric', month: 'short' }).format(new Date(atMs));
}

/** "same time", "+5h 30m", "−3h" compared with the viewer's own zone. */
export function diffLabel(zone: string, localZone: string, atMs: number): string {
  const d = offsetMinutes(zone, atMs) - offsetMinutes(localZone, atMs);
  if (d === 0) return 'same time';
  const sign = d > 0 ? '+' : '−';
  const a = Math.abs(d);
  const h = Math.floor(a / 60);
  const m = a % 60;
  return `${sign}${h > 0 ? `${h}h` : ''}${h > 0 && m > 0 ? ' ' : ''}${m > 0 ? `${m}m` : ''}`;
}

/** Milliseconds until the next whole minute (the clocks only need to redraw then). */
export const msToNextMinute = (nowMs: number): number => 60000 - (nowMs % 60000) + 20;

export const systemZone = (): string => Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
