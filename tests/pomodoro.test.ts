import { describe, expect, it } from 'vitest';
import {
  afterFinish, afterSkip, applyFinish, applySkip, AUTOSTART_WINDOW_MS, DEFAULT_DATA, dayKey, normalize, phaseMs, progress,
  pruneHistory, recordSession, shouldAutoStart, todayCount, type PomodoroData,
} from '../src/hub/widgets/pomodoro/logic';

const data = (over: Partial<PomodoroData> = {}): PomodoroData => ({ ...DEFAULT_DATA, ...over });

describe('pomodoro cycle', () => {
  it('runs focus, short, focus, short, focus, short, focus, long, then starts a new set', () => {
    let phase: 'focus' | 'short' | 'long' = 'focus';
    let cycle = 0;
    const seen: string[] = [];
    for (let i = 0; i < 9; i++) {
      seen.push(phase);
      const n = afterFinish(phase, cycle, 4);
      phase = n.phase;
      cycle = n.cycle;
    }
    expect(seen).toEqual(['focus', 'short', 'focus', 'short', 'focus', 'short', 'focus', 'long', 'focus']);
  });
  it('counts only finished focus sessions', () => {
    expect(afterFinish('focus', 0, 4).countedFocus).toBe(true);
    expect(afterFinish('short', 1, 4).countedFocus).toBe(false);
    expect(afterFinish('long', 4, 4).countedFocus).toBe(false);
  });
  it('keeps the set full during the long break and resets after it', () => {
    expect(afterFinish('focus', 3, 4)).toMatchObject({ phase: 'long', cycle: 4 });
    expect(afterFinish('long', 4, 4)).toMatchObject({ phase: 'focus', cycle: 0 });
  });
  it('honours a different long-break interval', () => {
    expect(afterFinish('focus', 1, 2)).toMatchObject({ phase: 'long', cycle: 2 });
  });
  it('skipping never counts a session or advances the set', () => {
    expect(afterSkip('focus', 1, 4)).toEqual({ phase: 'short', cycle: 1 });
    expect(afterSkip('focus', 3, 4)).toEqual({ phase: 'long', cycle: 3 });
    expect(afterSkip('short', 2, 4)).toEqual({ phase: 'focus', cycle: 2 });
    expect(afterSkip('long', 4, 4)).toEqual({ phase: 'focus', cycle: 0 });
    const d = applySkip(data({ cycle: 1 }));
    expect(d.history).toEqual({});
  });
  it('applyFinish records today and clears the timer id', () => {
    const at = new Date(2026, 10, 22, 15, 0).getTime();
    const d = applyFinish(data({ timerId: 7 }), at);
    expect(d).toMatchObject({ phase: 'short', cycle: 1, timerId: null });
    expect(todayCount(d.history, at)).toBe(1);
    expect(todayCount(applyFinish(d, at).history, at)).toBe(1); // finishing a break does not count
  });
});

describe('pomodoro document', () => {
  it('survives garbage and clamps values', () => {
    expect(normalize(null)).toEqual(DEFAULT_DATA);
    expect(normalize('x')).toEqual(DEFAULT_DATA);
    const d = normalize({ settings: { focusMin: 9999, shortMin: -3, longMin: 'a', longEvery: 1, autoStart: 'yes', sound: false }, phase: 'nope', cycle: 99, timerId: -2, history: { bad: 3, '2026-01-01': 'x', '2026-01-02': 4 } });
    expect(d.settings).toEqual({ focusMin: 180, shortMin: 1, longMin: 15, longEvery: 2, autoStart: false, sound: false });
    expect(d.phase).toBe('focus');
    expect(d.cycle).toBe(2);
    expect(d.timerId).toBeNull();
    expect(d.history).toEqual({ '2026-01-02': 4 });
  });
  it('keeps a valid document unchanged', () => {
    const d = data({ phase: 'long', cycle: 4, timerId: 3, history: { '2026-11-21': 5 } });
    expect(normalize(JSON.parse(JSON.stringify(d)))).toEqual(d);
  });
  it('converts minutes to milliseconds per phase', () => {
    expect(phaseMs('focus', DEFAULT_DATA.settings)).toBe(25 * 60_000);
    expect(phaseMs('short', DEFAULT_DATA.settings)).toBe(5 * 60_000);
    expect(phaseMs('long', DEFAULT_DATA.settings)).toBe(15 * 60_000);
  });
});

describe('pomodoro history', () => {
  it('uses the local calendar day', () => {
    expect(dayKey(new Date(2026, 0, 5, 23, 59).getTime())).toBe('2026-01-05');
    expect(dayKey(new Date(2026, 0, 6, 0, 1).getTime())).toBe('2026-01-06');
  });
  it('keeps only the last 30 days', () => {
    let h: Record<string, number> = {};
    for (let i = 1; i <= 40; i++) h = recordSession(h, new Date(2026, 0, i, 12).getTime());
    expect(Object.keys(h)).toHaveLength(30);
    expect(h['2026-01-01']).toBeUndefined();
    expect(h['2026-01-40' as string]).toBeUndefined();
    expect(Object.keys(pruneHistory({ '2026-01-01': 1, '2026-01-02': 1 }, 1))).toEqual(['2026-01-02']);
  });
  it('counts several sessions on one day', () => {
    const at = new Date(2026, 5, 1, 9).getTime();
    expect(todayCount(recordSession(recordSession({}, at), at + 3600_000), at)).toBe(2);
  });
});

describe('pomodoro auto-start', () => {
  const on = data({ settings: { ...DEFAULT_DATA.settings, autoStart: true } });
  it('starts the next phase only when enabled and the end was just noticed', () => {
    expect(shouldAutoStart(on, 0)).toBe(true);
    expect(shouldAutoStart(on, AUTOSTART_WINDOW_MS)).toBe(true);
    expect(shouldAutoStart(on, AUTOSTART_WINDOW_MS + 1)).toBe(false); // the Hub was closed or the PC slept
    expect(shouldAutoStart(data(), 0)).toBe(false);
  });
});

describe('progress ring', () => {
  it('is 0 at the start, 1 at the end and bounded', () => {
    expect(progress(1000, 1000)).toBe(0);
    expect(progress(1000, 250)).toBe(0.75);
    expect(progress(1000, 0)).toBe(1);
    expect(progress(1000, 5000)).toBe(0);
    expect(progress(0, 0)).toBe(0);
  });
});
