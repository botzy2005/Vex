/** Calendar rules. Pure (dates are plain "YYYY-MM-DD" strings and `today` is passed in), so month layout,
 *  leap years, ordering and limits are unit-tested without a browser. */

export interface CalEvent {
  id: string;
  /** Local calendar day, "2026-11-23". */
  date: string;
  /** "HH:MM" (24 hour), or null for an all-day event. */
  time: string | null;
  title: string;
  note: string;
}

export interface CalDoc {
  version: 1;
  /** First day of the week in the month grid. */
  weekStart: 'mon' | 'sun';
  next: number;
  events: CalEvent[];
}

export const MAX_EVENTS = 500;
export const MAX_TITLE = 80;
export const MAX_NOTE = 300;
export const EMPTY_DOC: CalDoc = { version: 1, weekStart: 'mon', next: 1, events: [] };

const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;
export const isLeap = (y: number): boolean => (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
export const daysInMonth = (y: number, m: number): number => [31, isLeap(y) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1];

/** A real calendar date between years 1900 and 2200. */
export function isValidDate(s: unknown): s is string {
  if (typeof s !== 'string') return false;
  const m = DATE_RE.exec(s);
  if (!m) return false;
  const y = Number(m[1]);
  const mo = Number(m[2]);
  const d = Number(m[3]);
  return y >= 1900 && y <= 2200 && mo >= 1 && mo <= 12 && d >= 1 && d <= daysInMonth(y, mo);
}

export const isValidTime = (s: unknown): s is string => typeof s === 'string' && /^([01]\d|2[0-3]):[0-5]\d$/.test(s);

const pad = (n: number): string => String(n).padStart(2, '0');
export const ymd = (y: number, m: number, d: number): string => `${y}-${pad(m)}-${pad(d)}`;
const parts = (s: string): [number, number, number] => {
  const m = DATE_RE.exec(s)!;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
};
const dayNo = (s: string): number => {
  const [y, m, d] = parts(s);
  return Math.floor(Date.UTC(y, m - 1, d) / 86400000);
};
const fromDayNo = (n: number): string => {
  const d = new Date(n * 86400000);
  return ymd(d.getUTCFullYear(), d.getUTCMonth() + 1, d.getUTCDate());
};

export const addDays = (s: string, n: number): string => fromDayNo(dayNo(s) + n);
export const daysBetween = (from: string, to: string): number => dayNo(to) - dayNo(from);
/** 0 = Monday ... 6 = Sunday. */
export const weekdayMon0 = (s: string): number => (((dayNo(s) + 3) % 7) + 7) % 7;

export function addMonths(y: number, m: number, n: number): { y: number; m: number } {
  const idx = y * 12 + (m - 1) + n;
  return { y: Math.floor(idx / 12), m: (idx % 12) + 1 };
}

export interface Cell {
  date: string;
  inMonth: boolean;
}

/** The six weeks (42 days) shown for a month, starting on the configured weekday. */
export function monthGrid(y: number, m: number, weekStart: 'mon' | 'sun'): Cell[] {
  const first = ymd(y, m, 1);
  const lead = weekStart === 'mon' ? weekdayMon0(first) : (weekdayMon0(first) + 1) % 7;
  const start = addDays(first, -lead);
  return Array.from({ length: 42 }, (_, i) => {
    const date = addDays(start, i);
    return { date, inMonth: Number(date.slice(5, 7)) === m && Number(date.slice(0, 4)) === y };
  });
}

export const WEEKDAYS_MON = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
export const weekdayNames = (weekStart: 'mon' | 'sun'): string[] => (weekStart === 'mon' ? WEEKDAYS_MON : ['Sun', ...WEEKDAYS_MON.slice(0, 6)]);
export const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];

export function normalize(raw: unknown): CalDoc {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const seen = new Set<string>();
  const events: CalEvent[] = [];
  if (Array.isArray(r.events)) {
    for (const e of r.events) {
      if (!e || typeof e !== 'object') continue;
      const o = e as Record<string, unknown>;
      if (typeof o.id !== 'string' || !/^e\d{1,9}$/.test(o.id) || seen.has(o.id) || !isValidDate(o.date) || typeof o.title !== 'string') continue;
      const title = o.title.trim().slice(0, MAX_TITLE);
      if (title === '') continue;
      seen.add(o.id);
      events.push({ id: o.id, date: o.date, time: isValidTime(o.time) ? o.time : null, title, note: typeof o.note === 'string' ? o.note.slice(0, MAX_NOTE) : '' });
      if (events.length >= MAX_EVENTS) break;
    }
  }
  const highest = events.reduce((mx, e) => Math.max(mx, Number(e.id.slice(1))), 0);
  const next = typeof r.next === 'number' && Number.isInteger(r.next) && r.next > highest ? r.next : highest + 1;
  return { version: 1, weekStart: r.weekStart === 'sun' ? 'sun' : 'mon', next, events };
}

export type EventInput = { date: string; time: string | null; title: string; note?: string };

export function validate(i: EventInput): string | null {
  if (i.title.trim() === '') return 'Give the event a title.';
  if (!isValidDate(i.date)) return 'Choose a valid date.';
  if (i.time !== null && !isValidTime(i.time)) return 'Choose a valid time, or leave it empty for an all-day event.';
  return null;
}

export function addEvent(doc: CalDoc, i: EventInput): { doc: CalDoc; id: string } | { error: string } {
  const bad = validate(i);
  if (bad) return { error: bad };
  if (doc.events.length >= MAX_EVENTS) return { error: `You can keep up to ${MAX_EVENTS} events. Delete some to add more.` };
  const id = `e${doc.next}`;
  const ev: CalEvent = { id, date: i.date, time: i.time, title: i.title.trim().slice(0, MAX_TITLE), note: (i.note ?? '').slice(0, MAX_NOTE) };
  return { doc: { ...doc, next: doc.next + 1, events: [...doc.events, ev] }, id };
}

export function updateEvent(doc: CalDoc, id: string, i: EventInput): { doc: CalDoc } | { error: string } {
  const bad = validate(i);
  if (bad) return { error: bad };
  if (!doc.events.some((e) => e.id === id)) return { error: 'That event no longer exists.' };
  return { doc: { ...doc, events: doc.events.map((e) => (e.id === id ? { ...e, date: i.date, time: i.time, title: i.title.trim().slice(0, MAX_TITLE), note: (i.note ?? '').slice(0, MAX_NOTE) } : e)) } };
}

export const removeEvent = (doc: CalDoc, id: string): CalDoc => (doc.events.some((e) => e.id === id) ? { ...doc, events: doc.events.filter((e) => e.id !== id) } : doc);

/** All-day events first, then by time, then by title. */
export const byTime = (a: CalEvent, b: CalEvent): number => (a.time ?? '').localeCompare(b.time ?? '') || a.title.localeCompare(b.title);

export const eventsOn = (events: CalEvent[], date: string): CalEvent[] => events.filter((e) => e.date === date).sort(byTime);

/** Events from `from` for the next `days` days (including `from`), in date and time order. */
export function upcoming(events: CalEvent[], from: string, days: number): CalEvent[] {
  const last = addDays(from, days - 1);
  return events.filter((e) => e.date >= from && e.date <= last).sort((a, b) => a.date.localeCompare(b.date) || byTime(a, b));
}

/** Dates in a month that have at least one event. */
export function busyDays(events: CalEvent[], y: number, m: number): Set<string> {
  const prefix = `${y}-${pad(m)}-`;
  return new Set(events.filter((e) => e.date.startsWith(prefix)).map((e) => e.date));
}

/** "Today", "Tomorrow", "In 5 days", "Yesterday", "3 days ago". */
export function relativeDay(date: string, today: string): string {
  const d = daysBetween(today, date);
  if (d === 0) return 'Today';
  if (d === 1) return 'Tomorrow';
  if (d === -1) return 'Yesterday';
  return d > 0 ? `In ${d} days` : `${-d} days ago`;
}

export function longDate(date: string): string {
  const [y, m, d] = parts(date);
  return `${WEEKDAYS_MON[weekdayMon0(date)]} ${d} ${MONTHS[m - 1]} ${y}`;
}

/** Local today as "YYYY-MM-DD" from a clock value. */
export function todayOf(ms: number): string {
  const d = new Date(ms);
  return ymd(d.getFullYear(), d.getMonth() + 1, d.getDate());
}

/** Milliseconds until local midnight (the "today" highlight needs to move then). */
export function msToMidnight(ms: number): number {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1).getTime() - ms + 50;
}
