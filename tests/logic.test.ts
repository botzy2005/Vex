import { describeItem, formatBytes, sensitiveNote, statusLine } from '../src/clipboard/format';
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

import { estimatedSingMs, headPosition, letterLevels } from '../src/lyrics/sync';

describe('letter sweep', () => {
  const plain = { timeMs: 1000, text: 'Hello world' };
  it('starts unlit, ends fully lit', () => {
    expect(letterLevels(11, 0).every((v) => v === 0)).toBe(true);
    expect(letterLevels(11, 11).every((v) => v === 1)).toBe(true);
  });
  it('lights letters left to right with a soft edge', () => {
    const l = letterLevels(10, 5);
    for (let i = 1; i < l.length; i++) expect(l[i]).toBeLessThanOrEqual(l[i - 1]);
    expect(l[0]).toBe(1);
    expect(l[9]).toBe(0);
    expect(l.some((v) => v > 0 && v < 1)).toBe(true);
  });
  it('estimates plain-LRC timing, capped by the line slot', () => {
    expect(estimatedSingMs(10, 10_000)).toBe(1200);
    expect(estimatedSingMs(100, 3000)).toBe(3000);
    expect(estimatedSingMs(1, 10_000)).toBe(570);
  });
  it('plain line: head is 0 before, n after, and monotonic', () => {
    expect(headPosition(plain.text, plain, 9000, 500)).toBe(0);
    expect(headPosition(plain.text, plain, 9000, 20_000)).toBe(11);
    const a = headPosition(plain.text, plain, 9000, 1500);
    const b = headPosition(plain.text, plain, 9000, 1800);
    expect(b).toBeGreaterThan(a);
  });
  it('word-timed line: head follows the real words, not an estimate', () => {
    const line = {
      timeMs: 0,
      text: 'Hello world',
      words: [
        { timeMs: 0, text: 'Hello ' },
        { timeMs: 3000, text: 'world' },
      ],
    };
    expect(headPosition(line.text, line, 6000, 1500)).toBeCloseTo(2.5, 5); // halfway through "Hello" (5 letters)
    expect(headPosition(line.text, line, 6000, 3000)).toBeGreaterThanOrEqual(5);
    expect(headPosition(line.text, line, 6000, 6000)).toBe(11);
  });
  it('word timing that does not match the text falls back to the estimate', () => {
    const line = { timeMs: 0, text: 'Different', words: [{ timeMs: 0, text: 'Hello' }] };
    expect(headPosition(line.text, line, 9000, 20_000)).toBe(9);
  });
});

import { parseInline, parseMarkdown, isSafeUrl } from '../src/assistant/markdownParse';
import { buildContext, detectContextRequest } from '../src/assistant/context';

describe('markdown', () => {
  it('parses headings, paragraphs, lists and rules', () => {
    const b = parseMarkdown('# Title\n\nHello **world**.\n\n- a\n- b\n\n1. one\n2. two\n\n---');
    expect(b.map((x) => x.t)).toEqual(['h', 'p', 'ul', 'ol', 'hr']);
    expect((b[2] as { items: unknown[] }).items).toHaveLength(2);
  });
  it('parses fenced code and keeps its content literal', () => {
    const b = parseMarkdown('```ts\nconst a = **1**;\n```\nafter');
    expect(b[0]).toEqual({ t: 'code', lang: 'ts', s: 'const a = **1**;' });
    expect(b[1].t).toBe('p');
  });
  it('an unfinished code fence (mid-stream) still renders as code', () => {
    const b = parseMarkdown('Here:\n```py\nprint(1)');
    expect(b[1]).toEqual({ t: 'code', lang: 'py', s: 'print(1)' });
  });
  it('inline: bold, italic, code and nesting', () => {
    expect(parseInline('a **b *c*** d')).toEqual([
      { t: 'text', s: 'a ' },
      { t: 'b', c: [{ t: 'text', s: 'b ' }, { t: 'i', c: [{ t: 'text', s: 'c' }] }] },
      { t: 'text', s: ' d' },
    ]);
    expect(parseInline('use `x**y`')[1]).toEqual({ t: 'code', s: 'x**y' });
  });
  it('unmatched markers stay literal (streaming)', () => {
    expect(parseInline('half **bold')).toEqual([{ t: 'text', s: 'half **bold' }]);
    expect(parseInline('snake_case_name is fine')).toEqual([{ t: 'text', s: 'snake_case_name is fine' }]);
    expect(parseInline('2 * 3 * 4')).toEqual([{ t: 'text', s: '2 * 3 * 4' }]);
  });
  it('only http(s) and mailto links are ever produced', () => {
    expect(isSafeUrl('https://example.com/a?b=1')).toBe(true);
    expect(isSafeUrl('javascript:alert(1)')).toBe(false);
    expect(isSafeUrl('data:text/html,hi')).toBe(false);
    expect(parseInline('[x](javascript:alert(1))')).toEqual([{ t: 'text', s: '[x](javascript:alert(1))' }]);
    expect(parseInline('[docs](https://example.com)')[0]).toMatchObject({ t: 'a', href: 'https://example.com' });
  });
  it('raw HTML in a reply is text, never markup', () => {
    const b = parseMarkdown('<img src=x onerror=alert(1)>');
    expect(b).toEqual([{ t: 'p', c: [{ t: 'text', s: '<img src=x onerror=alert(1)>' }] }]);
  });
});

describe('app context', () => {
  const session = { title: 'Night Drive', artist: 'The Testers', album: 'LP', durationMs: 200000, status: 'playing' } as never;
  const lyrics = { source: 'LRCLIB', lines: [{ timeMs: 0, text: 'line one' }, { timeMs: 1000, text: 'line two' }] };
  it('detects questions about the music, and only those', () => {
    expect(detectContextRequest('What song is playing?')).toEqual({ song: true, lyrics: false });
    expect(detectContextRequest("who sings this song")).toMatchObject({ song: true });
    expect(detectContextRequest('Explain these lyrics')).toEqual({ song: true, lyrics: true });
    expect(detectContextRequest('what do the lyrics mean')).toMatchObject({ lyrics: true });
    expect(detectContextRequest('Write a poem about the sea')).toEqual({ song: false, lyrics: false });
    expect(detectContextRequest('How do I play a song on guitar?')).toEqual({ song: false, lyrics: false });
  });
  it('attaches nothing unless asked', () => {
    expect(buildContext({ song: false, lyrics: false }, { session, lyrics: null, activeIndex: -1, positionMs: 0 })).toBeNull();
  });
  it('song context has title, artist and state', () => {
    const c = buildContext({ song: true, lyrics: false }, { session, lyrics: null, activeIndex: -1, positionMs: 65000 })!;
    expect(c.label).toBe('Now playing');
    expect(c.text).toContain('"Night Drive" by The Testers');
    expect(c.text).toContain('playing (1:05 of 3:20)');
    expect(c.text).not.toContain('Lyrics');
  });
  it('lyrics context includes the lines and marks the current one', () => {
    const c = buildContext({ song: true, lyrics: true }, { session, lyrics, activeIndex: 1, positionMs: 1500 })!;
    expect(c.label).toBe('Now playing + Lyrics');
    expect(c.text).toContain('line two"');
    expect(c.text).toContain('line one\nline two');
  });
  it('is honest when nothing is playing or no lyrics exist', () => {
    expect(buildContext({ song: true, lyrics: true }, { session: null, lyrics: null, activeIndex: -1, positionMs: 0 })!.text).toContain('nothing is playing');
    expect(buildContext({ song: false, lyrics: true }, { session, lyrics: null, activeIndex: -1, positionMs: 0 })!.text).toContain('no lyrics loaded');
  });
});

describe('music context detection (what the person asks decides what is shared)', () => {
  const asks = [
    'What song am I currently listening to?', 'What song is this?', "what's playing", "What's playing right now?", 'what am I listening to',
    'Which song is playing right now', 'which track is this', 'what music is playing', 'now playing?', 'who sings this', 'who is singing',
    'who is the artist', "who's this by?", 'who wrote this song', 'tell me about this song', 'how long is this song', 'is the music paused',
    'Tell me about the current track',
  ];
  const asksLyrics = [
    'explain these lyrics', 'Explain the lyrics', 'what is he singing', 'what are they saying',
    'what does this song mean', 'what is this song about', 'meaning of this song', 'explain this song', 'what does the current song mean',
  ];
  const unrelated = [
    'Explain cloud security in simple terms.', 'Write a song about cats', 'compose a song for my mom', 'write lyrics for a rap about pizza',
    'recommend some songs for studying', 'play some music', 'My favourite song ever is Hey Jude', 'how do I make music with a laptop',
    "what's playing at the cinema tonight", 'what music should I learn on guitar', 'who wrote Hamlet', 'what song is based on a true story',
    'what is a lyric poem', 'Compare cloud security and network security in a table', 'what is the meaning of life',
  ];
  it.each(asks)('attaches the song for: %s', (t) => {
    expect(detectContextRequest(t).song).toBe(true);
  });
  it.each(asksLyrics)('attaches song and lyrics for: %s', (t) => {
    expect(detectContextRequest(t)).toEqual({ song: true, lyrics: true });
  });
  it.each(unrelated)('attaches nothing for: %s', (t) => {
    expect(detectContextRequest(t)).toEqual({ song: false, lyrics: false });
  });
  it('uses unsynced lyrics when no synced lyrics exist, and says so when there are none', () => {
    const session = { title: 'T', artist: 'A', status: 'playing', durationMs: 1000 } as never;
    const withPlain = buildContext({ song: true, lyrics: true }, { session, lyrics: null, plainLyrics: 'la la la', activeIndex: -1, positionMs: 0 });
    expect(withPlain?.text).toContain('la la la');
    expect(withPlain?.text).toContain('"T" by A');
    const none = buildContext({ song: true, lyrics: true }, { session, lyrics: null, plainLyrics: null, activeIndex: -1, positionMs: 0 });
    expect(none?.text).toContain('no lyrics loaded');
  });
  it('builds nothing when nothing was asked for', () => {
    expect(buildContext({ song: false, lyrics: false }, { session: null, lyrics: null, activeIndex: -1, positionMs: 0 })).toBeNull();
  });
});

// ---------------------------------------------------------------- notifications
import { accessMessage, groupEntries, nextPreview, relativeTime } from '../src/notifications/group';
import type { NotificationApp, NotificationEntry } from '../src/shared/types';

const ne = (id: number, appId: string, tsMs: number, unread = true, title = `t${id}`): NotificationEntry => ({ id, appId, appName: appId.toUpperCase(), title, body: '', tsMs, unread });
const napps: NotificationApp[] = [{ id: 'a', name: 'A', icon: 'data:image/png;base64,AA', blocked: false }, { id: 'b', name: 'B', icon: null, blocked: false }];

describe('notification grouping', () => {
  const entries = [ne(1, 'a', 100), ne(2, 'b', 300), ne(3, 'a', 200, false), ne(4, 'b', 50)];

  it('groups by app, newest group first and newest item first', () => {
    const g = groupEntries(entries, true, napps);
    expect(g.map((x) => x.appId)).toEqual(['b', 'a']);
    expect(g[0].items.map((e) => e.id)).toEqual([2, 4]);
    expect(g[1].items.map((e) => e.id)).toEqual([3, 1]);
    expect(g[1].unread).toBe(1);
    expect(g[1].latestMs).toBe(200);
    expect(g[1].icon).toBe('data:image/png;base64,AA');
    expect(g[0].icon).toBeNull();
  });

  it('with grouping off every notification stands alone in time order', () => {
    const g = groupEntries(entries, false, napps);
    expect(g.map((x) => x.items[0].id)).toEqual([2, 3, 1, 4]);
    expect(g.every((x) => x.items.length === 1)).toBe(true);
  });

  it('copes with nothing and with apps missing from the app list', () => {
    expect(groupEntries([], true, [])).toEqual([]);
    expect(groupEntries([ne(1, 'zz', 5)], true, [])[0].icon).toBeNull();
  });

  it('does not mutate its input', () => {
    const copy = structuredClone(entries);
    groupEntries(entries, true, napps);
    expect(entries).toEqual(copy);
  });
});

describe('notification preview rule', () => {
  it('the first view is only a baseline, never a preview', () => {
    const s = nextPreview(null, [ne(5, 'a', 1)]);
    expect(s).toEqual({ maxId: 5, preview: null });
  });
  it('previews the newest new unread entry', () => {
    const base = nextPreview(null, [ne(1, 'a', 1)]);
    const s = nextPreview(base, [ne(3, 'b', 3), ne(2, 'a', 2), ne(1, 'a', 1)]);
    expect(s.preview?.id).toBe(3);
    expect(s.maxId).toBe(3);
  });
  it('does not preview again for views with nothing new, or for entries already read', () => {
    const base = { maxId: 3, preview: null };
    expect(nextPreview(base, [ne(3, 'a', 3)]).preview).toBeNull();
    expect(nextPreview(base, [ne(4, 'a', 4, false)]).preview).toBeNull();
  });
  it('clearing history does not lower the baseline (ids never come back)', () => {
    const s = nextPreview({ maxId: 9, preview: null }, []);
    expect(s.maxId).toBe(9);
  });
});

describe('notification text helpers', () => {
  it('formats relative time', () => {
    const now = 10_000_000;
    expect(relativeTime(now - 10_000, now)).toBe('now');
    expect(relativeTime(now + 5000, now)).toBe('now');
    expect(relativeTime(now - 5 * 60_000, now)).toBe('5 min ago');
    expect(relativeTime(now - 3 * 3_600_000, now)).toBe('3 h ago');
    expect(relativeTime(now - 2 * 86_400_000, now)).toBe('2 d ago');
  });
  it('explains every state in plain words', () => {
    expect(accessMessage({ state: 'allowed' }, true)).toBeNull();
    expect(accessMessage({ state: 'allowed' }, false)).toMatch(/off/i);
    expect(accessMessage({ state: 'denied' }, true)).toMatch(/Privacy/);
    expect(accessMessage({ state: 'unavailable', reason: 'No identity.' }, true)).toBe('No identity.');
    expect(accessMessage({ state: 'idle' }, true)).toMatch(/Waiting/);
  });
});

describe('notification layout', () => {
  const base = { assistant: 'idle' as const, hasSession: true, lyricsLineVisible: true };
  it('a notification preview beats lyrics and media but not an active assistant', () => {
    expect(resolveCompactView({ ...base, notification: true })).toBe('notification');
    expect(resolveCompactView({ ...base, assistant: 'thinking', notification: true })).toBe('assistant');
    expect(resolveCompactView({ ...base, hasSession: false, lyricsLineVisible: false, notification: true })).toBe('notification');
    expect(resolveCompactView({ ...base, notification: false })).toBe('lyrics');
  });
  it('has sizes for the preview and the panel', () => {
    const l = computeLayout({ expanded: false, tab: 'music', compact: 'notification', sizeScale: 1, shape: 'pill', cornerRadius: 26 });
    expect(l.view).toBe('notification');
    expect(l.width).toBeGreaterThan(300);
    expect(computeLayout({ expanded: true, tab: 'notifications', compact: 'idle', sizeScale: 1, shape: 'pill', cornerRadius: 26 }).view).toBe('expanded');
  });
});

describe('clipboard helpers', () => {
  const text = { id: 1, tsMs: 0, pinned: false, kind: 'text' as const, preview: 'a', chars: 1, lines: 1, width: 0, height: 0, bytes: 0, thumb: null };
  it('formats sizes', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2048)).toBe('2 KB');
    expect(formatBytes(3 * 1024 * 1024)).toBe('3.0 MB');
  });
  it('describes text and images', () => {
    expect(describeItem(text)).toBe('1 character');
    expect(describeItem({ ...text, chars: 1234, lines: 3 })).toBe('3 lines · 1,234 characters');
    expect(describeItem({ ...text, kind: 'image', width: 800, height: 600, bytes: 40960 })).toBe('Image · 800×600 · 40 KB');
  });
  const st = { enabled: true, paused: false, recording: true, count: 4, pinned: 1, imageBytes: 0, skippedSensitive: 0, note: null, persist: false };
  it('always says whether the history is listening', () => {
    expect(statusLine({ ...st, enabled: false, recording: false })).toEqual({ tone: 'off', text: 'Clipboard history is off' });
    expect(statusLine({ ...st, paused: true, recording: false }).tone).toBe('paused');
    expect(statusLine(st)).toEqual({ tone: 'recording', text: 'Recording copies · 4 saved' });
  });
  it('words the private-copy notice', () => {
    expect(sensitiveNote(0)).toBeNull();
    expect(sensitiveNote(1)).toContain('1 copy');
    expect(sensitiveNote(3)).toContain('3 copies');
    expect(sensitiveNote(3)).toContain('were not saved');
  });
  it('has a clipboard tab size', () => {
    const l = computeLayout({ expanded: true, tab: 'clipboard', compact: 'idle', sizeScale: 1, shape: 'pill', cornerRadius: 26 });
    expect(l.view).toBe('expanded');
    expect(l.baseHeight).toBeGreaterThan(200);
  });
});

import { onAccent } from '../src/island/accent';
describe('onAccent', () => {
  it('chooses readable text for light and dark accents', () => {
    expect(onAccent('#a5b4fc')).toBe('#101114'); // the default pastel accent needs dark text
    expect(onAccent('#ffd60a')).toBe('#101114');
    expect(onAccent('#0a3d91')).toBe('#ffffff');
    expect(onAccent('#000000')).toBe('#ffffff');
    expect(onAccent('not a colour')).toBe('#101114');
  });
});
