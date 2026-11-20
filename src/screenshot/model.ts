// Pure geometry and data for the screenshot editor. No DOM here, so all of it is unit-tested.

export type Tool = 'crop' | 'arrow' | 'line' | 'rect' | 'ellipse' | 'text' | 'highlight' | 'blur' | 'pixelate';

export interface Pt {
  x: number;
  y: number;
}
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Shape =
  | { type: 'line' | 'arrow'; a: Pt; b: Pt; color: string; width: number }
  | { type: 'rect' | 'ellipse'; box: Box; color: string; width: number }
  | { type: 'highlight'; box: Box; color: string }
  | { type: 'blur' | 'pixelate'; box: Box; strength: number }
  | { type: 'text'; at: Pt; text: string; color: string; size: number };

/** The most pixels an edited image may have (keeps memory and the save step safe). */
export const MAX_EDIT_PIXELS = 40_000_000;
export const MAX_EDIT_SIDE = 16_000;
/** A drag shorter than this is a click, not a shape. */
export const MIN_DRAG = 3;

export function normBox(a: Pt, b: Pt): Box {
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(a.x - b.x), h: Math.abs(a.y - b.y) };
}

/** Keep a box inside the image (whole pixels). Null when nothing is left. */
export function clampBox(b: Box, width: number, height: number): Box | null {
  const x0 = Math.max(0, Math.floor(b.x));
  const y0 = Math.max(0, Math.floor(b.y));
  const x1 = Math.min(width, Math.ceil(b.x + b.w));
  const y1 = Math.min(height, Math.ceil(b.y + b.h));
  return x1 - x0 >= 1 && y1 - y0 >= 1 ? { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } : null;
}

/** A drag between two image points as a usable box, or null if it is only a click. */
export function dragBox(a: Pt, b: Pt, width: number, height: number): Box | null {
  const n = normBox(a, b);
  if (n.w < MIN_DRAG || n.h < MIN_DRAG) return null;
  return clampBox(n, width, height);
}

export const dist = (a: Pt, b: Pt) => Math.hypot(a.x - b.x, a.y - b.y);

/** The three corners of an arrow head at `b`, pointing away from `a`; `base` is the middle of its back edge. */
export function arrowHead(a: Pt, b: Pt, width: number): { tip: Pt; left: Pt; right: Pt; base: Pt } {
  const len = Math.max(width * 4, 10);
  const total = dist(a, b);
  const angle = Math.atan2(b.y - a.y, b.x - a.x);
  // A short arrow still gets a head, but never longer than the arrow itself.
  const l = Math.min(len, Math.max(total, 1));
  const spread = (28 * Math.PI) / 180;
  const left = { x: b.x - l * Math.cos(angle - spread), y: b.y - l * Math.sin(angle - spread) };
  const right = { x: b.x - l * Math.cos(angle + spread), y: b.y - l * Math.sin(angle + spread) };
  const base = { x: (left.x + right.x) / 2, y: (left.y + right.y) / 2 };
  return { tip: b, left, right, base };
}

/** Line widths and text sizes are chosen in screen-friendly units; scale them up for big screenshots. */
export function sizeScale(width: number, height: number): number {
  return Math.max(1, Math.max(width, height) / 1100);
}

/** Blur radius / pixel block that makes covered text unreadable, scaled with the image. */
export function effectStrength(kind: 'blur' | 'pixelate', width: number, height: number): number {
  const m = Math.max(width, height);
  return kind === 'blur' ? Math.min(40, Math.max(10, Math.round(m / 110))) : Math.min(48, Math.max(8, Math.round(m / 90)));
}

export function validateSize(w: number, h: number): string | null {
  if (!Number.isFinite(w) || !Number.isFinite(h) || !Number.isInteger(w) || !Number.isInteger(h)) return 'Enter whole numbers.';
  if (w < 1 || h < 1) return 'The size must be at least 1 pixel.';
  if (w > MAX_EDIT_SIDE || h > MAX_EDIT_SIDE) return `The longest side can be at most ${MAX_EDIT_SIDE.toLocaleString('en-US')} pixels.`;
  if (w * h > MAX_EDIT_PIXELS) return 'That is too many pixels to edit safely.';
  return null;
}

/** Change one side of a size, keeping the proportions when `lock` is on. */
export function resizeFrom(orig: { w: number; h: number }, side: 'w' | 'h', value: number, lock: boolean, current: { w: number; h: number }): { w: number; h: number } {
  const v = Math.round(value);
  if (!lock) return side === 'w' ? { w: v, h: current.h } : { w: current.w, h: v };
  return side === 'w' ? { w: v, h: Math.max(1, Math.round((v * orig.h) / orig.w)) } : { w: Math.max(1, Math.round((v * orig.w) / orig.h)), h: v };
}

/** Size after turning by quarter turns (1 = 90° clockwise). */
export function rotatedSize(w: number, h: number, turns: number): { w: number; h: number } {
  return Math.abs(turns) % 2 === 1 ? { w: h, h: w } : { w, h };
}

/** Map a pointer position (client pixels) to image pixels, for a canvas shown at a different size. */
export function toImagePoint(client: Pt, rect: { left: number; top: number; width: number; height: number }, imgW: number, imgH: number): Pt {
  const x = ((client.x - rect.left) / Math.max(rect.width, 1)) * imgW;
  const y = ((client.y - rect.top) / Math.max(rect.height, 1)) * imgH;
  return { x: Math.min(imgW, Math.max(0, x)), y: Math.min(imgH, Math.max(0, y)) };
}

/** And back: an image point to client pixels (to place the text box). */
export function toClientPoint(p: Pt, rect: { left: number; top: number; width: number; height: number }, imgW: number, imgH: number): Pt {
  return { x: rect.left + (p.x / imgW) * rect.width, y: rect.top + (p.y / imgH) * rect.height };
}

/** Build the shape a drag makes, or null when it is too small to mean anything. */
export function shapeFromDrag(tool: Exclude<Tool, 'crop' | 'text'>, a: Pt, b: Pt, o: { color: string; width: number; imgW: number; imgH: number }): Shape | null {
  const s = sizeScale(o.imgW, o.imgH);
  if (tool === 'line' || tool === 'arrow') {
    return dist(a, b) < MIN_DRAG ? null : { type: tool, a, b, color: o.color, width: o.width * s };
  }
  const box = dragBox(a, b, o.imgW, o.imgH);
  if (!box) return null;
  if (tool === 'rect' || tool === 'ellipse') return { type: tool, box, color: o.color, width: o.width * s };
  if (tool === 'highlight') return { type: 'highlight', box, color: o.color };
  return { type: tool, box, strength: effectStrength(tool, o.imgW, o.imgH) };
}
