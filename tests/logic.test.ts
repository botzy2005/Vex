import { describe, expect, it } from 'vitest';
import { parseLrc } from '../src/lyrics/lrc';
import { currentPositionMs, findActiveIndex, lineEndMs, wordFractions } from '../src/lyrics/sync';
import { MOTION_PRESETS, Spring } from '../src/island/spring';
import { computeLayout, resolveCompactView, WINDOW_PAD } from '../src/island/layout';
import { pickAccent } from '../src/island/accent';

// Lyrics below are made-up placeholder text, not real song lyrics.
const SAMPLE = `[ti:Test Track]
[ar:Nobody]
[00:01.00]first line
[00:03.50]second line
[00:03.50][00:10.00]shared stamp line
[00:12.5]short fraction
[01:00.123]long fraction
`;

describe('parseLrc', () => {
  it('parses timestamps, ignores metadata and sorts', () => {
    const doc = parseLrc(SAMPLE)!;
    expect(doc.lines.map((l) => l.timeMs)).toEqual([1000, 3500, 3500, 10000, 12500, 60123]);
    expect(doc.lines[0].text).toBe('first line');
  });

  it('supports several timestamps on one line', () => {
    const doc = parseLrc(SAMPLE)!;
    expect(doc.lines.filter((l) => l.text === 'shared stamp line').map((l) => l.timeMs)).toEqual([3500, 10000]);
  });

  it('handles 1/2/3-digit fractions', () => {
    const doc = parseLrc('[00:01.5]a\n[00:02.50]b\n[00:03.500]c\n[00:04]d')!;
    expect(doc.lines.map((l) => l.timeMs)).toEqual([1500, 2500, 3500, 4000]);
  });

  it('applies the [offset:] tag (positive = earlier)', () => {
    const doc = parseLrc('[offset:500]\n[00:02.00]x')!;
    expect(doc.lines[0].timeMs).toBe(1500);
  });

  it('returns null for text without timestamps', () => {
    expect(parseLrc('just words\nno times')).toBeNull();
    expect(parseLrc('')).toBeNull();
  });

  it('keeps empty lines as instrumental gaps', () => {
    const doc = parseLrc('[00:01.00]a\n[00:05.00]\n[00:09.00]b')!;
    expect(doc.lines[1].text).toBe('');
  });

  it('handles CRLF and a BOM', () => {
    const doc = parseLrc('﻿[00:01.00]a\r\n[00:02.00]b\r\n')!;
    expect(doc.lines).toHaveLength(2);
  });

  it('parses enhanced word timing', () => {
    const doc = parseLrc('[00:10.00]<00:10.00>one <00:10.50>two <00:11.00>three')!;
    const line = doc.lines[0];
    expect(line.text).toBe('one two three');
    expect(line.words!.map((w) => w.timeMs)).toEqual([10000, 10500, 11000]);
    expect(line.words!.map((w) => w.text).join('')).toBe('one two three');
  });

  it('plain lines have no words array', () => {
    expect(parseLrc('[00:01.00]plain')!.lines[0].words).toBeUndefined();
  });
});

describe('lyric sync', () => {
  const lines = parseLrc('[00:01.00]a\n[00:03.00]b\n[00:05.00]c')!.lines;

  it('finds the active line', () => {
    expect(findActiveIndex(lines, 0)).toBe(-1);
    expect(findActiveIndex(lines, 1000)).toBe(0);
    expect(findActiveIndex(lines, 2999)).toBe(0);
    expect(findActiveIndex(lines, 3000)).toBe(1);
    expect(findActiveIndex(lines, 999999)).toBe(2);
  });

  it('is correct for every position (binary search vs linear scan)', () => {
    const many = Array.from({ length: 200 }, (_, i) => ({ timeMs: i * 1700 + 300, text: String(i) }));
    for (let p = 0; p < 360000; p += 137) {
      let expected = -1;
      for (let i = 0; i < many.length; i++) if (many[i].timeMs <= p) expected = i;
      expect(findActiveIndex(many, p)).toBe(expected);
    }
  });

  it('computes line end', () => {
    expect(lineEndMs(lines, 0)).toBe(3000);
    expect(lineEndMs(lines, 2)).toBe(10000);
  });

  it('computes karaoke fractions', () => {
    const line = parseLrc('[00:10.00]<00:10.00>a <00:11.00>b')!.lines[0];
    expect(wordFractions(line, 12000, 9000)).toEqual([0, 0]);
    expect(wordFractions(line, 12000, 10500)).toEqual([0.5, 0]);
    expect(wordFractions(line, 12000, 11500)).toEqual([1, 0.5]);
    expect(wordFractions(line, 12000, 13000)).toEqual([1, 1]);
  });

  it('extrapolates playback position only while playing', () => {
    const base = { positionMs: 10000, positionUpdatedAt: 1000, playbackRate: 1, durationMs: 200000 };
    expect(currentPositionMs({ ...base, status: 'playing' }, 3000)).toBe(12000);
    expect(currentPositionMs({ ...base, status: 'paused' }, 3000)).toBe(10000);
    expect(currentPositionMs({ ...base, status: 'playing', playbackRate: 2 }, 3000)).toBe(14000);
    expect(currentPositionMs({ ...base, status: 'playing', durationMs: 11000 }, 5000)).toBe(11000);
  });
});

describe('spring', () => {
  const run = (cfg: { stiffness: number; damping: number }, from: number, to: number) => {
    const s = new Spring(from);
    s.target = to;
    let peak = from;
    let t = 0;
    while (!s.atRest(0.01) && t < 5) {
      s.step(1 / 60, cfg);
      peak = Math.max(peak, s.x);
      t += 1 / 60;
    }
    return { s, peak, t };
  };

  const settleTime = (cfg: { stiffness: number; damping: number }, eps: number) => {
    const s = new Spring(100);
    s.target = 400;
    let t = 0;
    while (!s.atRest(eps) && t < 5) {
      s.step(1 / 60, cfg);
      t += 1 / 60;
    }
    return { x: s.x, t };
  };

  it('every preset is visually settled (within half a pixel) in under 1.2 s', () => {
    for (const cfg of Object.values(MOTION_PRESETS)) {
      expect(settleTime(cfg, 0.5).t).toBeLessThan(1.2);
    }
  });

  it('every preset fully converges to its target', () => {
    for (const cfg of Object.values(MOTION_PRESETS)) {
      const { x, t } = settleTime(cfg, 0.01);
      expect(Math.abs(x - 400)).toBeLessThan(0.05);
      expect(t).toBeLessThan(3);
    }
  });

  it('subtle does not overshoot, bouncy does', () => {
    expect(run(MOTION_PRESETS.subtle, 100, 400).peak).toBeLessThanOrEqual(400.5);
    expect(run(MOTION_PRESETS.bouncy, 100, 400).peak).toBeGreaterThan(410);
  });

  it('is independent of frame rate within tolerance', () => {
    const a = new Spring(0); a.target = 100;
    const b = new Spring(0); b.target = 100;
    for (let i = 0; i < 30; i++) a.step(1 / 30, MOTION_PRESETS.normal);
    for (let i = 0; i < 120; i++) b.step(1 / 120, MOTION_PRESETS.normal);
    expect(Math.abs(a.x - b.x)).toBeLessThan(0.5);
  });

  it('snap jumps straight to the target', () => {
    const s = new Spring(0); s.target = 50; s.snap();
    expect(s.x).toBe(50);
    expect(s.atRest()).toBe(true);
  });
});

describe('island layout', () => {
  it('prioritises assistant > lyrics > media > idle', () => {
    expect(resolveCompactView({ assistant: 'thinking', hasSession: true, lyricsLineVisible: true })).toBe('assistant');
    expect(resolveCompactView({ assistant: 'idle', hasSession: true, lyricsLineVisible: true })).toBe('lyrics');
    expect(resolveCompactView({ assistant: 'idle', hasSession: true, lyricsLineVisible: false })).toBe('media');
    expect(resolveCompactView({ assistant: 'idle', hasSession: false, lyricsLineVisible: true })).toBe('idle');
  });

  const base = { expanded: false, tab: 'music' as const, compact: 'idle' as const, sizeScale: 1, shape: 'pill' as const, cornerRadius: 26 };

  it('collapsed island is a full pill; expanded uses the corner radius', () => {
    const c = computeLayout(base);
    expect(c.radius).toBe(c.height / 2);
    const e = computeLayout({ ...base, expanded: true });
    expect(e.radius).toBe(26);
    expect(e.width).toBeGreaterThan(c.width);
    expect(e.height).toBeGreaterThan(c.height);
  });

  it('notch shape flattens the top corners', () => {
    expect(computeLayout({ ...base, shape: 'notch' }).radiusTop).toBe(0);
    expect(computeLayout(base).radiusTop).toBeGreaterThan(0);
  });

  it('scales every dimension and keeps the window larger than the island', () => {
    const a = computeLayout(base);
    const b = computeLayout({ ...base, sizeScale: 1.4 });
    expect(b.width).toBeGreaterThan(a.width);
    expect(a.windowWidth).toBe(a.width + WINDOW_PAD * 2);
    expect(a.windowHeight).toBeGreaterThan(a.height);
  });
});

describe('accent extraction', () => {
  const solid = (r: number, g: number, b: number, a = 255, n = 64) =>
    new Uint8ClampedArray(Array.from({ length: n }, () => [r, g, b, a]).flat());

  it('picks the vivid colour', () => {
    const acc = pickAccent(solid(220, 30, 40))!;
    expect(parseInt(acc.slice(1, 3), 16)).toBeGreaterThan(200);
  });

  it('ignores greys, black and transparent pixels', () => {
    expect(pickAccent(solid(128, 128, 128))).toBeNull();
    expect(pickAccent(solid(0, 0, 0))).toBeNull();
    expect(pickAccent(solid(255, 0, 0, 0))).toBeNull();
  });

  it('prefers the dominant hue over a minority one', () => {
    const red = Array.from(solid(230, 20, 20, 255, 40));
    const blue = Array.from(solid(20, 20, 230, 255, 10));
    const acc = pickAccent(new Uint8ClampedArray([...red, ...blue]))!;
    expect(parseInt(acc.slice(1, 3), 16)).toBeGreaterThan(parseInt(acc.slice(5, 7), 16));
  });

  it('lifts colours that are too dark for a black background', () => {
    const acc = pickAccent(solid(90, 20, 20))!;
    expect(parseInt(acc.slice(1, 3), 16)).toBeGreaterThanOrEqual(150);
  });
});
