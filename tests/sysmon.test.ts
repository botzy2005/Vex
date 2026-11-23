import { describe, expect, it } from 'vitest';
import { barWidth, batteryLabel, formatBytes, formatRate, level, normalize } from '../src/hub/widgets/sysmon/logic';

describe('sysmon logic', () => {
  it('formats rates', () => {
    expect(formatRate(null)).toBe('–');
    expect(formatRate(0)).toBe('0 B/s');
    expect(formatRate(340_000)).toBe('340 KB/s');
    expect(formatRate(1_200_000)).toBe('1.2 MB/s');
  });
  it('formats bytes', () => {
    expect(formatBytes(9_800_000_000)).toBe('9.8 GB');
    expect(formatBytes(512_000_000)).toBe('512 MB');
  });
  it('levels', () => {
    expect([level(null), level(69), level(70), level(89), level(90)]).toEqual(['ok', 'ok', 'warn', 'warn', 'high']);
  });
  it('battery wording', () => {
    expect(batteryLabel({ percent: 50, charging: true, plugged: true, minutesLeft: null })).toBe('Charging');
    expect(batteryLabel({ percent: 100, charging: false, plugged: true, minutesLeft: null })).toBe('Plugged in');
    expect(batteryLabel({ percent: 40, charging: false, plugged: false, minutesLeft: 125 })).toBe('On battery · 2 h 5 min left');
    expect(batteryLabel({ percent: 40, charging: false, plugged: false, minutesLeft: null })).toBe('On battery');
  });
  it('bar width is clamped', () => {
    expect([barWidth(null), barWidth(-5), barWidth(42.4), barWidth(180), barWidth(NaN)]).toEqual([0, 0, 42, 100, 0]);
  });
  it('normalises the saved interval', () => {
    expect(normalize(null).intervalMs).toBe(2000);
    expect(normalize({ intervalMs: 5000 }).intervalMs).toBe(5000);
    expect(normalize({ intervalMs: 123 }).intervalMs).toBe(2000);
  });
});
