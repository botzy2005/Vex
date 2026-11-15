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
