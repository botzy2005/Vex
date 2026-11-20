import { describe, expect, it } from 'vitest';
import { canRedo, canUndo, createHistory, HISTORY_CAP, push, redo, undo } from '../src/screenshot/history';
import {
  arrowHead, clampBox, dragBox, effectStrength, normBox, resizeFrom, rotatedSize, shapeFromDrag, sizeScale,
  toClientPoint, toImagePoint, validateSize,
} from '../src/screenshot/model';
import { resolveCompactView } from '../src/island/layout';

describe('screenshot geometry', () => {
  it('normalises a drag in any direction', () => {
    expect(normBox({ x: 10, y: 20 }, { x: 4, y: 50 })).toEqual({ x: 4, y: 20, w: 6, h: 30 });
  });
  it('clamps a box to the image and drops empty ones', () => {
    expect(clampBox({ x: -5, y: -5, w: 20, h: 20 }, 100, 100)).toEqual({ x: 0, y: 0, w: 15, h: 15 });
    expect(clampBox({ x: 95.2, y: 10, w: 20, h: 5 }, 100, 100)).toEqual({ x: 95, y: 10, w: 5, h: 5 });
    expect(clampBox({ x: 200, y: 0, w: 10, h: 10 }, 100, 100)).toBeNull();
  });
  it('treats a tiny drag as a click', () => {
    expect(dragBox({ x: 5, y: 5 }, { x: 6, y: 6 }, 100, 100)).toBeNull();
    expect(dragBox({ x: 5, y: 5 }, { x: 50, y: 40 }, 100, 100)).toEqual({ x: 5, y: 5, w: 45, h: 35 });
  });
  it('builds an arrow head that points at the end and is never longer than the arrow', () => {
    const h = arrowHead({ x: 0, y: 0 }, { x: 100, y: 0 }, 4);
    expect(h.tip).toEqual({ x: 100, y: 0 });
    expect(h.left.x).toBeLessThan(100);
    expect(h.left.y).toBeCloseTo(-h.right.y);
    const short = arrowHead({ x: 0, y: 0 }, { x: 3, y: 0 }, 10);
    expect(Math.hypot(short.left.x - 3, short.left.y)).toBeLessThanOrEqual(3 + 1e-9);
  });
  it('scales strokes only for large images', () => {
    expect(sizeScale(800, 600)).toBe(1);
    expect(sizeScale(2200, 1000)).toBe(2);
  });
  it('picks blur/pixelate strength within bounds', () => {
    for (const k of ['blur', 'pixelate'] as const) {
      expect(effectStrength(k, 100, 100)).toBeGreaterThanOrEqual(8);
      expect(effectStrength(k, 20000, 100)).toBeLessThanOrEqual(48);
    }
  });
  it('round-trips pointer and image coordinates', () => {
    const rect = { left: 10, top: 20, width: 400, height: 200 };
    const p = toImagePoint({ x: 210, y: 120 }, rect, 1600, 800);
    expect(p).toEqual({ x: 800, y: 400 });
    expect(toClientPoint(p, rect, 1600, 800)).toEqual({ x: 210, y: 120 });
    expect(toImagePoint({ x: -50, y: 9999 }, rect, 1600, 800)).toEqual({ x: 0, y: 800 });
  });
});

describe('resize and rotate', () => {
  it('validates sizes', () => {
    expect(validateSize(100, 50)).toBeNull();
    expect(validateSize(0, 50)).toMatch(/at least/);
    expect(validateSize(1.5, 5)).toMatch(/whole/);
    expect(validateSize(NaN, 5)).toMatch(/whole/);
    expect(validateSize(20000, 5)).toMatch(/longest/);
    expect(validateSize(10000, 10000)).toMatch(/too many/);
  });
  it('keeps proportions when locked', () => {
    const o = { w: 800, h: 400 };
    expect(resizeFrom(o, 'w', 400, true, o)).toEqual({ w: 400, h: 200 });
    expect(resizeFrom(o, 'h', 100, true, o)).toEqual({ w: 200, h: 100 });
    expect(resizeFrom(o, 'w', 400, false, o)).toEqual({ w: 400, h: 400 });
  });
  it('swaps the sides on a quarter turn', () => {
    expect(rotatedSize(30, 10, 1)).toEqual({ w: 10, h: 30 });
    expect(rotatedSize(30, 10, -1)).toEqual({ w: 10, h: 30 });
    expect(rotatedSize(30, 10, 2)).toEqual({ w: 30, h: 10 });
  });
});

describe('shapeFromDrag', () => {
  const o = { color: '#f00', width: 4, imgW: 2200, imgH: 1000 };
  it('ignores clicks', () => {
    expect(shapeFromDrag('rect', { x: 1, y: 1 }, { x: 2, y: 2 }, o)).toBeNull();
    expect(shapeFromDrag('line', { x: 1, y: 1 }, { x: 2, y: 1 }, o)).toBeNull();
  });
  it('scales stroke width with the image and carries the colour', () => {
    const s = shapeFromDrag('rect', { x: 0, y: 0 }, { x: 100, y: 100 }, o);
    expect(s).toMatchObject({ type: 'rect', color: '#f00', width: 8 });
  });
  it('makes blur and pixelate shapes with a strength', () => {
    expect(shapeFromDrag('blur', { x: 0, y: 0 }, { x: 50, y: 50 }, o)).toMatchObject({ type: 'blur' });
    expect(shapeFromDrag('pixelate', { x: 0, y: 0 }, { x: 50, y: 50 }, o)).toMatchObject({ type: 'pixelate' });
  });
});

describe('undo history', () => {
  it('undoes, redoes and drops redo on a new action', () => {
    let h = createHistory(0);
    expect(canUndo(h)).toBe(false);
    h = push(push(h, 1), 2);
    h = undo(h);
    expect(h.present).toBe(1);
    expect(canRedo(h)).toBe(true);
    h = redo(h);
    expect(h.present).toBe(2);
    h = undo(undo(h));
    h = push(h, 9);
    expect(canRedo(h)).toBe(false);
    expect(undo(h).present).toBe(0);
  });
  it('is a no-op at the ends and caps its length', () => {
    const h = createHistory('a');
    expect(undo(h)).toBe(h);
    expect(redo(h)).toBe(h);
    let big = createHistory(0);
    for (let i = 1; i <= HISTORY_CAP + 20; i++) big = push(big, i);
    expect(big.past.length).toBe(HISTORY_CAP);
  });
});

describe('island screenshot view', () => {
  const base = { assistant: 'idle' as const, hasSession: true, lyricsLineVisible: true };
  it('outranks notifications and media but not an active assistant', () => {
    expect(resolveCompactView({ ...base, screenshot: true, notification: true })).toBe('screenshot');
    expect(resolveCompactView({ ...base, assistant: 'thinking', screenshot: true })).toBe('assistant');
    expect(resolveCompactView({ ...base, screenshot: false })).toBe('lyrics');
  });
});

import { activePreset, PRESETS } from '../src/settings/presets';
import { effectiveGlass } from '../src/settings/IslandPreview';
describe('appearance presets', () => {
  const base = { theme: 'dark', shape: 'pill', glassOpacity: 0.92, cornerRadius: 26, accentSource: 'artwork', customAccent: '#A5B4FC', motion: 'normal', sizeScale: 1 } as const;
  it('every preset stays inside the settings ranges and has a valid colour', () => {
    expect(new Set(PRESETS.map((p) => p.id)).size).toBe(PRESETS.length);
    for (const p of PRESETS) {
      expect(p.look.glassOpacity).toBeGreaterThanOrEqual(0.4);
      expect(p.look.glassOpacity).toBeLessThanOrEqual(1);
      expect(p.look.cornerRadius).toBeGreaterThanOrEqual(8);
      expect(p.look.cornerRadius).toBeLessThanOrEqual(40);
      expect(p.look.customAccent).toMatch(/^#[0-9a-f]{6}$/);
    }
  });
  it('recognises the active preset and none after a tweak', () => {
    expect(activePreset({ ...base })).toBe('midnight');
    expect(activePreset({ ...base, cornerRadius: 27 })).toBeNull();
    expect(activePreset({ ...base, theme: 'light', accentSource: 'custom', customAccent: '#5b6cd6', glassOpacity: 0.96 })).toBe('daylight');
  });
  it('the preview surface keeps the same opacity floor as the island', () => {
    expect(effectiveGlass(0.4)).toBeGreaterThanOrEqual(0.88);
    expect(effectiveGlass(1)).toBe(1);
  });
});
