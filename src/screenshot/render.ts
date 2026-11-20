// Canvas drawing for the editor. The same functions draw the on-screen view and the exported image,
// so what you see is what you save.

import { arrowHead, type Box, type Shape } from './model';

export interface Doc {
  /** The picture the shapes sit on. Replaced (not changed) by crop, rotate and resize, so earlier states stay valid for undo. */
  base: HTMLCanvasElement;
  width: number;
  height: number;
  shapes: Shape[];
}

export function newCanvas(w: number, h: number): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  return c;
}

function ctx2d(c: HTMLCanvasElement): CanvasRenderingContext2D {
  const ctx = c.getContext('2d');
  if (!ctx) throw new Error('The canvas is not available.');
  return ctx;
}

export function canvasFromImage(img: HTMLImageElement): HTMLCanvasElement {
  const c = newCanvas(img.naturalWidth, img.naturalHeight);
  ctx2d(c).drawImage(img, 0, 0);
  return c;
}

const copyCanvas = (src: HTMLCanvasElement) => {
  const c = newCanvas(src.width, src.height);
  ctx2d(c).drawImage(src, 0, 0);
  return c;
};

function drawEffect(ctx: CanvasRenderingContext2D, kind: 'blur' | 'pixelate', box: Box, strength: number) {
  const snap = copyCanvas(ctx.canvas); // what is under the area right now, including earlier shapes
  ctx.save();
  ctx.beginPath();
  ctx.rect(box.x, box.y, box.w, box.h);
  ctx.clip();
  if (kind === 'blur') {
    ctx.filter = `blur(${strength}px)`;
    ctx.drawImage(snap, 0, 0);
  } else {
    const sw = Math.max(1, Math.ceil(box.w / strength));
    const sh = Math.max(1, Math.ceil(box.h / strength));
    const small = newCanvas(sw, sh);
    const sctx = ctx2d(small);
    sctx.imageSmoothingEnabled = true;
    sctx.drawImage(snap, box.x, box.y, box.w, box.h, 0, 0, sw, sh);
    ctx.imageSmoothingEnabled = false; // hard square blocks
    ctx.drawImage(small, 0, 0, sw, sh, box.x, box.y, sw * strength, sh * strength);
  }
  ctx.restore();
}

export function drawShape(ctx: CanvasRenderingContext2D, s: Shape) {
  ctx.save();
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  switch (s.type) {
    case 'line':
    case 'arrow': {
      ctx.strokeStyle = s.color;
      ctx.fillStyle = s.color;
      ctx.lineWidth = s.width;
      ctx.beginPath();
      ctx.moveTo(s.a.x, s.a.y);
      if (s.type === 'arrow') {
        const h = arrowHead(s.a, s.b, s.width);
        ctx.lineTo(h.base.x, h.base.y);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(h.tip.x, h.tip.y);
        ctx.lineTo(h.left.x, h.left.y);
        ctx.lineTo(h.right.x, h.right.y);
        ctx.closePath();
        ctx.fill();
      } else {
        ctx.lineTo(s.b.x, s.b.y);
        ctx.stroke();
      }
      break;
    }
    case 'rect':
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width;
      ctx.strokeRect(s.box.x, s.box.y, s.box.w, s.box.h);
      break;
    case 'ellipse':
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width;
      ctx.beginPath();
      ctx.ellipse(s.box.x + s.box.w / 2, s.box.y + s.box.h / 2, Math.max(s.box.w / 2, 0.5), Math.max(s.box.h / 2, 0.5), 0, 0, Math.PI * 2);
      ctx.stroke();
      break;
    case 'highlight':
      ctx.globalAlpha = 0.4;
      ctx.fillStyle = s.color;
      ctx.fillRect(s.box.x, s.box.y, s.box.w, s.box.h);
      break;
    case 'text': {
      ctx.fillStyle = s.color;
      ctx.font = `600 ${s.size}px "Segoe UI Variable", "Segoe UI", system-ui, sans-serif`;
      ctx.textBaseline = 'top';
      s.text.split('\n').forEach((line, i) => ctx.fillText(line, s.at.x, s.at.y + i * s.size * 1.25));
      break;
    }
    case 'blur':
    case 'pixelate':
      ctx.restore();
      drawEffect(ctx, s.type, s.box, s.strength);
      return;
  }
  ctx.restore();
}

export function renderDoc(ctx: CanvasRenderingContext2D, doc: Doc, extra: Shape[] = []) {
  ctx.clearRect(0, 0, doc.width, doc.height);
  ctx.drawImage(doc.base, 0, 0);
  for (const s of doc.shapes) drawShape(ctx, s);
  for (const s of extra) drawShape(ctx, s);
}

/** The finished picture: base plus every shape. */
export function flatten(doc: Doc): HTMLCanvasElement {
  const c = newCanvas(doc.width, doc.height);
  renderDoc(ctx2d(c), doc);
  return c;
}

/** The image of `doc` as PNG data (lossless). */
export function exportPng(doc: Doc): string {
  return flatten(doc).toDataURL('image/png');
}

const fresh = (c: HTMLCanvasElement): Doc => ({ base: c, width: c.width, height: c.height, shapes: [] });

export function cropDoc(doc: Doc, box: Box): Doc {
  const flat = flatten(doc);
  const out = newCanvas(box.w, box.h);
  ctx2d(out).drawImage(flat, box.x, box.y, box.w, box.h, 0, 0, box.w, box.h);
  return fresh(out);
}

/** Turn by quarter turns clockwise (negative = counter-clockwise). */
export function rotateDoc(doc: Doc, turns: 1 | -1): Doc {
  const flat = flatten(doc);
  const out = newCanvas(doc.height, doc.width);
  const ctx = ctx2d(out);
  ctx.translate(turns === 1 ? doc.height : 0, turns === 1 ? 0 : doc.width);
  ctx.rotate((turns * Math.PI) / 2);
  ctx.drawImage(flat, 0, 0);
  return fresh(out);
}

export function resizeDoc(doc: Doc, w: number, h: number): Doc {
  const flat = flatten(doc);
  const out = newCanvas(w, h);
  const ctx = ctx2d(out);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(flat, 0, 0, w, h);
  return fresh(out);
}

export const docFromCanvas = fresh;
