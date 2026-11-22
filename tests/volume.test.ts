import { describe, expect, it } from 'vitest';
import { clampPercent, stepPercent, volumeLabel, volumeLevel } from '../src/island/volume';

describe('volume helpers', () => {
  it('picks the icon level (same thresholds as the Rust core)', () => {
    expect(volumeLevel(80, true)).toBe('muted');
    expect(volumeLevel(0, false)).toBe('muted');
    expect(volumeLevel(1, false)).toBe('low');
    expect(volumeLevel(33, false)).toBe('low');
    expect(volumeLevel(34, false)).toBe('medium');
    expect(volumeLevel(66, false)).toBe('medium');
    expect(volumeLevel(67, false)).toBe('high');
    expect(volumeLevel(100, false)).toBe('high');
  });
  it('clamps garbage to 0-100', () => {
    expect(clampPercent(-4)).toBe(0);
    expect(clampPercent(140)).toBe(100);
    expect(clampPercent(Number.NaN)).toBe(0);
    expect(clampPercent(49.6)).toBe(50);
  });
  it('steps and stops at both ends', () => {
    expect(stepPercent(50, 5)).toBe(55);
    expect(stepPercent(98, 5)).toBe(100);
    expect(stepPercent(3, -5)).toBe(0);
  });
  it('labels muted and level', () => {
    expect(volumeLabel(30, true)).toBe('Muted');
    expect(volumeLabel(30, false)).toBe('30%');
  });
});
