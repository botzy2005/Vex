import { useEffect, useState, type RefObject } from 'react';
import type { Measure } from './fit';

/** Layout size of an element (before any CSS transform, so island scaling is already accounted for). */
export function useElementSize(ref: RefObject<HTMLElement | null>): { width: number; height: number } {
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const read = () => {
      const w = el.clientWidth;
      const h = el.clientHeight;
      setSize((s) => (s.width === w && s.height === h ? s : { width: w, height: h }));
    };
    read();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(read);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
}

let canvas: CanvasRenderingContext2D | null | undefined;

/** Measures text with the real font (canvas), linear in size so one 100px measurement serves every size. */
export function canvasMeasure(family: string, weight: number): Measure {
  if (canvas === undefined) canvas = document.createElement('canvas').getContext('2d');
  const ctx = canvas;
  if (!ctx) return (t, px) => t.length * px * 0.58; // no canvas: a conservative average glyph width
  ctx.font = `${weight} 100px ${family}`;
  const cache = new Map<string, number>();
  return (text, px) => {
    let w = cache.get(text);
    if (w === undefined) {
      ctx.font = `${weight} 100px ${family}`;
      w = ctx.measureText(text).width;
      if (cache.size > 400) cache.clear();
      cache.set(text, w);
    }
    return (w * px) / 100;
  };
}

/** Computed font family of an element (so measuring matches what is drawn). */
export function familyOf(el: HTMLElement | null): string {
  return el ? getComputedStyle(el).fontFamily || 'sans-serif' : 'sans-serif';
}
