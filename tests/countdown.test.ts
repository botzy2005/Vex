import { describe, expect, it } from 'vitest';
import { cleanLabel, describeLength, MAX_MS, mine, normalize, parseDuration, PRESETS, soonest } from '../src/hub/widgets/countdown/logic';
import type { HubTimer } from '../src/shared/types';

const t = (id: number, owner: string, extra: object): HubTimer => ({ id, owner, label: `t${id}`, durationMs: 60000, ...extra }) as HubTimer;

describe('countdown input', () => {
  it('turns h/m/s into milliseconds, treating blanks as zero', () => {
    expect(parseDuration('', '5', '')).toEqual({ ok: true, ms: 300_000 });
    expect(parseDuration('1', '30', '15')).toEqual({ ok: true, ms: 5_415_000 });
    expect(parseDuration('0', '0', '1')).toEqual({ ok: true, ms: 1000 });
    expect(parseDuration('', '90', '')).toEqual({ ok: true, ms: 5_400_000 }); // 90 minutes is fine
    expect(parseDuration('99', '59', '59')).toEqual({ ok: true, ms: MAX_MS });
  });
  it('rejects empty, negative, fractional-garbage and oversized input with a reason', () => {
    for (const [h, m, s] of [['', '', ''], ['0', '0', '0'], ['-1', '5', ''], ['x', '', ''], ['100', '', ''], ['', '', '999999999']]) {
      const r = parseDuration(h, m, s);
      expect(r.ok, `${h}:${m}:${s}`).toBe(false);
      if (!r.ok) expect(r.reason.length).toBeGreaterThan(10);
    }
    expect(parseDuration('', '2.9', '')).toEqual({ ok: true, ms: 120_000 }); // fractions are floored
  });
  it('tidies labels', () => {
    expect(cleanLabel('  Tea   brewing  ')).toBe('Tea brewing');
    expect(cleanLabel('x'.repeat(80))).toHaveLength(40);
  });
  it('describes lengths', () => {
    expect(describeLength(300_000)).toBe('5 min');
    expect(describeLength(5_415_000)).toBe('1 h 30 min 15 s');
    expect(describeLength(45_000)).toBe('45 s');
    expect(PRESETS.map((p) => describeLength(p.ms))).toEqual(['1 min', '5 min', '10 min', '15 min', '30 min', '1 h']);
  });
  it('repairs the saved document', () => {
    expect(normalize(null)).toEqual({ version: 1, sound: true });
    expect(normalize({ sound: false })).toEqual({ version: 1, sound: false });
    expect(normalize({ sound: 'no' })).toEqual({ version: 1, sound: true });
  });
});

describe('countdown timers', () => {
  const list = [t(3, 'countdown', { state: 'running', endsAt: 900 }), t(1, 'pomodoro', { state: 'running', endsAt: 100 }), t(2, 'countdown', { state: 'running', endsAt: 500 }), t(4, 'countdown', { state: 'idle' })];
  it('keeps only its own timers, oldest first', () => {
    expect(mine(list).map((x) => x.id)).toEqual([2, 3, 4]);
  });
  it('picks the running one that ends first, ignoring other owners', () => {
    expect(soonest(list)?.id).toBe(2);
    expect(soonest([t(4, 'countdown', { state: 'idle' })])).toBeNull();
    expect(soonest([])).toBeNull();
  });
});
