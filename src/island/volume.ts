/** Volume rules shared by the panel and the test stand-in. They mirror `vex_core::volume`, which is the rule
 *  the native side applies; the Rust tests cover the same cases. */

export type VolumeLevel = 'muted' | 'low' | 'medium' | 'high';

export function volumeLevel(percent: number, muted: boolean): VolumeLevel {
  if (muted || percent <= 0) return 'muted';
  if (percent < 34) return 'low';
  if (percent < 67) return 'medium';
  return 'high';
}

export function clampPercent(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return Math.max(0, Math.min(100, Math.round(v)));
}

/** New percent after a relative step of `delta`, kept within 0-100. */
export function stepPercent(current: number, delta: number): number {
  return clampPercent(current + delta);
}

/** Slider updates are sent at most this often while dragging (about 30 per second). */
export const SLIDER_INTERVAL_MS = 33;

export function volumeLabel(percent: number, muted: boolean): string {
  return muted ? 'Muted' : `${clampPercent(percent)}%`;
}
