/** Width in pixels of `text` when drawn at `px` font size. Injected so the logic can be tested without a browser. */
export type Measure = (text: string, px: number) => number;

/** How many lines `text` takes when wrapped greedily into `maxWidth`. Words wider than the box are broken by character. */
export function wrapLines(text: string, maxWidth: number, px: number, measure: Measure, breakWords = true): number {
  const t = text.trim();
  if (!t) return 1;
  if (maxWidth <= 0) return Infinity;
  const space = measure(' ', px);
  let lines = 1;
  let x = 0;
  for (const word of t.split(/\s+/)) {
    const w = measure(word, px);
    if (w > maxWidth) {
      if (!breakWords) return Infinity; // an unbreakable word that is too wide cannot fit at this size
      // Long word: it starts on a fresh line when something is already there, then fills lines character by character.
      if (x > 0) {
        lines++;
        x = 0;
      }
      for (const ch of word) {
        const cw = measure(ch, px);
        if (x + cw > maxWidth && x > 0) {
          lines++;
          x = 0;
        }
        x += cw;
      }
      x += space;
      continue;
    }
    if (x > 0 && x + w > maxWidth) {
      lines++;
      x = 0;
    }
    x += w + space;
  }
  return lines;
}

export interface FitOptions {
  maxWidth: number;
  maxHeight: number;
  /** Largest and smallest font size to use. */
  max: number;
  min: number;
  maxLines: number;
  lineHeight: number;
  measure: Measure;
  /** False when words must stay whole (per-letter animation), so a word wider than the box forces a smaller size. */
  breakWords?: boolean;
}

/** The largest font size (in 0.5px steps) at which `text` fits the box in at most `maxLines` lines. Falls back to `min`. */
export function fitFontSize(text: string, o: FitOptions): number {
  for (let s = o.max; s >= o.min; s -= 0.5) {
    const lines = wrapLines(text, o.maxWidth, s, o.measure, o.breakWords ?? true);
    if (lines <= o.maxLines && lines * s * o.lineHeight <= o.maxHeight) return s;
  }
  return o.min;
}

export interface StageInput {
  width: number;
  height: number;
  now: string;
  /** Whether a previous / next line is shown (they take height but are single, ellipsised lines). */
  hasPrev: boolean;
  hasNext: boolean;
  /** The biggest the current line may be. */
  cap: number;
  measure: Measure;
  breakWords?: boolean;
}

export interface StageSizes {
  now: number;
  side: number;
}

export const STAGE_MIN = 11;
const SIDE_RATIO = 0.78;
const SIDE_MIN = 10.5;
const SIDE_MAX = 13;
const GAP = 3;
const NOW_LINE_HEIGHT = 1.25;
const SIDE_LINE_HEIGHT = 1.3;

/**
 * Sizes for the three-line lyric stage: the current line as large as the stage allows (up to `cap`, at most two lines),
 * with small previous/next lines. Everything is guaranteed to fit `height` unless even the smallest size cannot.
 */
export function fitStage(i: StageInput): StageSizes {
  const sideFor = (now: number) => Math.min(SIDE_MAX, Math.max(SIDE_MIN, now * SIDE_RATIO));
  for (let s = i.cap; s >= STAGE_MIN; s -= 0.5) {
    const side = sideFor(s);
    const sideH = ((i.hasPrev ? 1 : 0) + (i.hasNext ? 1 : 0)) * side * SIDE_LINE_HEIGHT + GAP * 2;
    const nowBudget = i.height - sideH;
    const lines = wrapLines(i.now, i.width, s, i.measure, i.breakWords ?? true);
    if (lines <= 2 && lines * s * NOW_LINE_HEIGHT <= nowBudget) return { now: s, side };
  }
  return { now: STAGE_MIN, side: SIDE_MIN };
}

/** Base size for the scrolling lyrics list, from the panel width (about 38 characters per line), scaled by the user's size setting. */
export function listBaseSize(width: number, userSize: number): number {
  const mult = Math.min(1.25, Math.max(0.85, userSize / 14));
  const base = Math.min(15, Math.max(12, width / 38)) * mult;
  return Math.round(Math.min(18, Math.max(11.5, base)) * 2) / 2;
}

/** Cap for the current line in the player's three-line stage, from the user's size setting. */
export function peekCap(userSize: number): number {
  const mult = Math.min(1.25, Math.max(0.85, userSize / 14));
  return Math.round(Math.min(18, 15 * mult) * 2) / 2;
}
