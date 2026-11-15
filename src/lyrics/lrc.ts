import type { LyricLine, LyricWord, LyricsDoc } from '../shared/types';

const LINE_TIME = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g;
const WORD_TAG = /<(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?>/g;
const META_TAG = /^\[([A-Za-z#]+):(.*)\]\s*$/;

function toMs(min: string, sec: string, frac?: string): number {
  let ms = (parseInt(min, 10) * 60 + parseInt(sec, 10)) * 1000;
  if (frac) {
    // "5" -> 500 ms, "50" -> 500 ms, "500" -> 500 ms
    ms += parseInt(frac.padEnd(3, '0').slice(0, 3), 10);
  }
  return ms;
}

/** Parse the `<mm:ss.xx>` word tags of enhanced LRC. Returns undefined when there are none. */
function parseWords(raw: string, offsetMs: number): { text: string; words?: LyricWord[] } {
  WORD_TAG.lastIndex = 0;
  const tags = [...raw.matchAll(WORD_TAG)];
  if (tags.length === 0) return { text: raw.trim() };

  const words: LyricWord[] = [];
  let leading = raw.slice(0, tags[0].index);
  tags.forEach((m, i) => {
    const start = (m.index ?? 0) + m[0].length;
    const end = i + 1 < tags.length ? tags[i + 1].index : raw.length;
    const text = raw.slice(start, end);
    if (text.trim().length > 0) {
      words.push({ timeMs: Math.max(0, toMs(m[1], m[2], m[3]) - offsetMs), text });
    } else if (words.length > 0) {
      // trailing whitespace belongs to the previous word so spacing survives
      words[words.length - 1].text += text;
    }
  });
  leading = leading.trim();
  const text = (leading + ' ' + words.map((w) => w.text).join('')).trim();
  return { text, words: words.length > 0 ? words : undefined };
}

/**
 * Parse synced lyrics in LRC format (including multiple timestamps per line,
 * the `[offset:]` tag and enhanced per-word timing). Returns null when the
 * text contains no timestamps, i.e. it is not synced lyrics.
 */
export function parseLrc(input: string, source = 'lrc'): LyricsDoc | null {
  const text = input.replace(/^﻿/, '');
  let offsetMs = 0;
  const lines: LyricLine[] = [];

  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;

    const meta = META_TAG.exec(line);
    if (meta && !/^\d/.test(meta[1])) {
      if (meta[1].toLowerCase() === 'offset') {
        const n = parseInt(meta[2].trim(), 10);
        if (Number.isFinite(n)) offsetMs = n;
      }
      continue;
    }

    LINE_TIME.lastIndex = 0;
    const stamps: number[] = [];
    let end = 0;
    let m: RegExpExecArray | null;
    while ((m = LINE_TIME.exec(line)) && m.index === end) {
      stamps.push(toMs(m[1], m[2], m[3]));
      end = LINE_TIME.lastIndex;
    }
    if (stamps.length === 0) continue;

    const body = line.slice(end);
    for (const stamp of stamps) {
      // LRC spec: a positive [offset] makes lyrics appear sooner.
      const timeMs = Math.max(0, stamp - offsetMs);
      const parsed = parseWords(body, offsetMs);
      lines.push({ timeMs, text: parsed.text, ...(parsed.words ? { words: parsed.words } : {}) });
    }
  }

  if (lines.length === 0) return null;
  lines.sort((a, b) => a.timeMs - b.timeMs);
  return { lines, source };
}
