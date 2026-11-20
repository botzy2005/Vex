/** Dominant, vivid colour of an image: used to tint the island from album artwork. */

function rgbToHsv(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    if (max === r) h = ((g - b) / d) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return [h, max === 0 ? 0 : d / max, max / 255];
}

const hex = (n: number) => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, '0');

/**
 * Pick an accent from RGBA pixel data: bucket pixels by hue, weight by
 * saturation x brightness, take the heaviest bucket's average colour and lift
 * it if it would be too dark to read on a black island.
 * Returns null for images with no usable colour (e.g. all grey/black/transparent).
 */
export function pickAccent(data: Uint8ClampedArray | number[]): string | null {
  const BINS = 12;
  const weight = new Array<number>(BINS).fill(0);
  const sum = Array.from({ length: BINS }, () => [0, 0, 0, 0]);

  for (let i = 0; i + 3 < data.length; i += 4) {
    const a = data[i + 3];
    if (a < 128) continue;
    const [h, s, v] = rgbToHsv(data[i], data[i + 1], data[i + 2]);
    if (s < 0.25 || v < 0.2) continue;
    const bin = Math.floor(h / (360 / BINS)) % BINS;
    const w = s * v;
    weight[bin] += w;
    sum[bin][0] += data[i] * w;
    sum[bin][1] += data[i + 1] * w;
    sum[bin][2] += data[i + 2] * w;
    sum[bin][3] += w;
  }

  let best = -1;
  for (let b = 0; b < BINS; b++) if (best < 0 || weight[b] > weight[best]) best = b;
  if (best < 0 || sum[best][3] === 0) return null;

  let r = sum[best][0] / sum[best][3];
  let g = sum[best][1] / sum[best][3];
  let bl = sum[best][2] / sum[best][3];
  const peak = Math.max(r, g, bl);
  if (peak < 150) {
    const k = 150 / peak;
    r *= k;
    g *= k;
    bl *= k;
  }
  return `#${hex(r)}${hex(g)}${hex(bl)}`;
}

/** Load an image URL and extract its accent. Resolves null when it can't be read. */
export async function extractAccent(url: string): Promise<string | null> {
  try {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.src = url;
    await img.decode();
    const size = 32;
    const canvas = document.createElement('canvas');
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(img, 0, 0, size, size);
    return pickAccent(ctx.getImageData(0, 0, size, size).data);
  } catch {
    return null;
  }
}

export const DEFAULT_ACCENT = '#a5b4fc';

/** Text colour that stays readable on an accent-coloured button (dark on light accents, white on dark ones). */
export function onAccent(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return '#101114';
  const n = parseInt(m[1], 16);
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const L = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
  // Pick whichever of near-black / white has the higher contrast ratio.
  return (L + 0.05) / (0.006 + 0.05) >= 1.05 / (L + 0.05) ? '#101114' : '#ffffff';
}
