import type { LyricLine } from '../shared/types';

/** Index of the line being sung at `posMs`, or -1 before the first line. */
export function findActiveIndex(lines: LyricLine[], posMs: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let ans = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].timeMs <= posMs) {
      ans = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return ans;
}

/** When the line ends: the next line's start, or a short tail after the last line. */
export function lineEndMs(lines: LyricLine[], index: number, tailMs = 5000): number {
  const next = lines[index + 1];
  return next ? next.timeMs : lines[index].timeMs + tailMs;
}

/**
 * Karaoke fill (0..1) for each word of a line at `posMs`. A word fills from its
 * own start to the next word's start (the last word fills until `endMs`).
 */
export function wordFractions(line: LyricLine, endMs: number, posMs: number): number[] {
  const words = line.words ?? [];
  return words.map((w, i) => {
    const start = w.timeMs;
    const stop = i + 1 < words.length ? words[i + 1].timeMs : Math.max(endMs, start + 1);
    if (posMs <= start) return 0;
    if (posMs >= stop) return 1;
    return (posMs - start) / (stop - start);
  });
}

/** Extrapolate a player-reported position to "now" so the UI doesn't wait for the next poll. */
export function currentPositionMs(
  s: { positionMs: number; positionUpdatedAt: number; playbackRate: number; status: string; durationMs: number | null },
  nowMs: number,
): number {
  let pos = s.positionMs;
  if (s.status === 'playing') pos += Math.max(0, nowMs - s.positionUpdatedAt) * (s.playbackRate || 1);
  if (s.durationMs != null) pos = Math.min(pos, s.durationMs);
  return Math.max(0, pos);
}

// ---- per-letter sweep ---------------------------------------------------------

/** Width, in letters, of the soft edge of the sweep. */
export const LETTER_EDGE = 2.5;

/**
 * How long we assume a line takes to sing when the source has no word timing:
 * about 70 ms per letter plus a short lead, never longer than the line's slot.
 * This is an ESTIMATE; plain LRC only says when a line starts.
 */
export function estimatedSingMs(chars: number, slotMs: number): number {
  return Math.max(400, Math.min(slotMs, chars * 70 + 500));
}

/**
 * Where the "singing head" is, in letters (0..chars). With enhanced-LRC word
 * timing the head follows the real words; otherwise it moves linearly over an
 * estimated singing time.
 */
export function headPosition(text: string, line: LyricLine, endMs: number, posMs: number): number {
  const chars = Array.from(text);
  const n = chars.length;
  if (n === 0) return 0;
  const words = line.words;
  if (words && words.length > 0) {
    const joined = words.map((w) => w.text).join('');
    const lead = joined.length - joined.trimStart().length;
    if (joined.trim() === text) {
      const f = wordFractions(line, endMs, posMs);
      let head = 0;
      let cursor = -lead;
      words.forEach((w, i) => {
        const len = w.text.trimEnd().length - (i === 0 ? lead : 0);
        const start = Math.max(0, cursor);
        if (f[i] > 0) head = start + f[i] * Math.max(0, len);
        cursor += w.text.length;
      });
      return Math.min(n, head);
    }
  }
  const span = estimatedSingMs(n, Math.max(1, endMs - line.timeMs));
  const p = Math.min(1, Math.max(0, (posMs - line.timeMs) / span));
  return p * n;
}

/** Lit amount (0..1) of each letter given the head position; the edge is soft so letters glow in. */
export function letterLevels(count: number, head: number, edge = LETTER_EDGE): number[] {
  if (count === 0) return [];
  const scaled = (head / count) * (count + edge);
  return Array.from({ length: count }, (_, k) => Math.min(1, Math.max(0, (scaled - k) / edge)));
}
