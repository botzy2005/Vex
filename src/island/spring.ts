import type { Motion } from '../shared/types';

export interface SpringConfig {
  stiffness: number;
  damping: number;
  mass?: number;
}

/** Spring presets for the "animation intensity" setting. `off` is handled by snapping. */
export const MOTION_PRESETS: Record<Exclude<Motion, 'off'>, SpringConfig> = {
  subtle: { stiffness: 300, damping: 34 }, // ~critically damped, no overshoot
  normal: { stiffness: 260, damping: 22 }, // gentle overshoot
  bouncy: { stiffness: 220, damping: 14 }, // visible bounce
};

export class Spring {
  x: number;
  v = 0;
  target: number;

  constructor(initial: number) {
    this.x = initial;
    this.target = initial;
  }

  /** Advance by `dt` seconds (sub-stepped for stability). */
  step(dt: number, cfg: SpringConfig): void {
    const mass = cfg.mass ?? 1;
    const maxStep = 1 / 240;
    let remaining = Math.min(dt, 0.1);
    while (remaining > 1e-6) {
      const h = Math.min(maxStep, remaining);
      const a = (-cfg.stiffness * (this.x - this.target) - cfg.damping * this.v) / mass;
      this.v += a * h;
      this.x += this.v * h;
      remaining -= h;
    }
  }

  snap(): void {
    this.x = this.target;
    this.v = 0;
  }

  atRest(eps = 0.05): boolean {
    return Math.abs(this.x - this.target) < eps && Math.abs(this.v) < eps;
  }
}
