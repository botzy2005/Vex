/** Pomodoro rules. Pure, so the cycle, the counting and the stored document are unit-tested without a browser. */

export type Phase = 'focus' | 'short' | 'long';

export interface PomodoroSettings {
  focusMin: number;
  shortMin: number;
  longMin: number;
  /** A long break comes after this many focus sessions. */
  longEvery: number;
  /** Start the next phase by itself when one ends while the Hub is open. Off by default. */
  autoStart: boolean;
  /** Play a chime when a phase ends. */
  sound: boolean;
}

export interface PomodoroData {
  version: 1;
  settings: PomodoroSettings;
  phase: Phase;
  /** Focus sessions finished in the current set (0..longEvery). */
  cycle: number;
  /** The shared timer that belongs to the current phase. */
  timerId: number | null;
  /** Finished focus sessions per local day ("2026-11-22"), the last 30 days only. */
  history: Record<string, number>;
}

export const DEFAULT_SETTINGS: PomodoroSettings = { focusMin: 25, shortMin: 5, longMin: 15, longEvery: 4, autoStart: false, sound: true };

export const DEFAULT_DATA: PomodoroData = { version: 1, settings: DEFAULT_SETTINGS, phase: 'focus', cycle: 0, timerId: null, history: {} };

export const LIMITS = { focusMin: [1, 180], shortMin: [1, 60], longMin: [1, 120], longEvery: [2, 8] } as const;
export const HISTORY_DAYS = 30;
/** A phase that ended this recently while the Hub was open may auto-start the next one; older ends never do. */
export const AUTOSTART_WINDOW_MS = 15_000;

const num = (v: unknown, lo: number, hi: number, fallback: number): number =>
  typeof v === 'number' && Number.isFinite(v) ? Math.max(lo, Math.min(hi, Math.round(v))) : fallback;

export function normalizeSettings(raw: unknown): PomodoroSettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const d = DEFAULT_SETTINGS;
  return {
    focusMin: num(r.focusMin, ...LIMITS.focusMin, d.focusMin),
    shortMin: num(r.shortMin, ...LIMITS.shortMin, d.shortMin),
    longMin: num(r.longMin, ...LIMITS.longMin, d.longMin),
    longEvery: num(r.longEvery, ...LIMITS.longEvery, d.longEvery),
    autoStart: typeof r.autoStart === 'boolean' ? r.autoStart : d.autoStart,
    sound: typeof r.sound === 'boolean' ? r.sound : d.sound,
  };
}

/** Repair whatever was stored (older version, hand edit, garbage) into a usable document. */
export function normalize(raw: unknown): PomodoroData {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const settings = normalizeSettings(r.settings);
  const phase: Phase = r.phase === 'short' || r.phase === 'long' ? r.phase : 'focus';
  const history: Record<string, number> = {};
  if (r.history && typeof r.history === 'object') {
    for (const [k, v] of Object.entries(r.history as Record<string, unknown>)) {
      if (/^\d{4}-\d{2}-\d{2}$/.test(k) && typeof v === 'number' && Number.isFinite(v) && v > 0) history[k] = Math.min(999, Math.floor(v));
    }
  }
  return {
    version: 1,
    settings,
    phase,
    cycle: num(r.cycle, 0, settings.longEvery, 0),
    timerId: typeof r.timerId === 'number' && Number.isInteger(r.timerId) && r.timerId > 0 ? r.timerId : null,
    history: pruneHistory(history),
  };
}

export function phaseMs(phase: Phase, s: PomodoroSettings): number {
  const min = phase === 'focus' ? s.focusMin : phase === 'short' ? s.shortMin : s.longMin;
  return min * 60_000;
}

export const PHASE_LABEL: Record<Phase, string> = { focus: 'Focus', short: 'Short break', long: 'Long break' };

export interface Step {
  phase: Phase;
  cycle: number;
}

/** What follows `phase` when it ran to its end. Finishing a focus session counts it. */
export function afterFinish(phase: Phase, cycle: number, longEvery: number): Step & { countedFocus: boolean } {
  if (phase === 'focus') {
    const c = Math.min(longEvery, cycle + 1);
    return { phase: c >= longEvery ? 'long' : 'short', cycle: c, countedFocus: true };
  }
  return { phase: 'focus', cycle: phase === 'long' ? 0 : cycle, countedFocus: false };
}

/** What follows `phase` when the user skips it: nothing is counted, and skipping focus does not advance the set. */
export function afterSkip(phase: Phase, cycle: number, longEvery: number): Step {
  if (phase === 'focus') return { phase: cycle + 1 >= longEvery ? 'long' : 'short', cycle };
  const n = afterFinish(phase, cycle, longEvery);
  return { phase: n.phase, cycle: n.cycle };
}

/** Local calendar day as "YYYY-MM-DD" (a session belongs to the day it finished on the user's clock). */
export function dayKey(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function pruneHistory(h: Record<string, number>, keep = HISTORY_DAYS): Record<string, number> {
  const keys = Object.keys(h).sort().slice(-keep);
  return Object.fromEntries(keys.map((k) => [k, h[k]]));
}

export function recordSession(h: Record<string, number>, atMs: number): Record<string, number> {
  const k = dayKey(atMs);
  return pruneHistory({ ...h, [k]: Math.min(999, (h[k] ?? 0) + 1) });
}

export function todayCount(h: Record<string, number>, nowMs: number): number {
  return h[dayKey(nowMs)] ?? 0;
}

/** The document after the current phase ended at `atMs` (counts a focus session and moves to the next phase). */
export function applyFinish(d: PomodoroData, atMs: number): PomodoroData {
  const n = afterFinish(d.phase, d.cycle, d.settings.longEvery);
  return { ...d, phase: n.phase, cycle: n.cycle, timerId: null, history: n.countedFocus ? recordSession(d.history, atMs) : d.history };
}

export function applySkip(d: PomodoroData): PomodoroData {
  const n = afterSkip(d.phase, d.cycle, d.settings.longEvery);
  return { ...d, phase: n.phase, cycle: n.cycle, timerId: null };
}

/** Whether the next phase should start by itself: only when enabled and the end was just noticed live. */
export function shouldAutoStart(d: PomodoroData, lateMs: number): boolean {
  return d.settings.autoStart && lateMs >= 0 && lateMs <= AUTOSTART_WINDOW_MS;
}

/** Fraction of the phase that is done, 0..1 (for the progress ring). */
export function progress(durationMs: number, remaining: number): number {
  if (durationMs <= 0) return 0;
  return Math.max(0, Math.min(1, 1 - remaining / durationMs));
}
