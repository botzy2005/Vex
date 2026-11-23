/** Pure rules for the System Monitor card (the numbers themselves come from the native side). */
import type { SysBattery } from '../../../shared/types';

export const INTERVALS = [1000, 2000, 5000] as const;
export type Interval = (typeof INTERVALS)[number];

export interface SysDoc {
  version: 1;
  intervalMs: Interval;
}
export const DEFAULT_DOC: SysDoc = { version: 1, intervalMs: 2000 };

export function normalize(raw: unknown): SysDoc {
  const r = raw as { intervalMs?: unknown } | null;
  const v = r && typeof r === 'object' ? r.intervalMs : undefined;
  return { version: 1, intervalMs: (INTERVALS as readonly unknown[]).includes(v) ? (v as Interval) : 2000 };
}

export function formatRate(bps: number | null): string {
  if (bps === null || !Number.isFinite(bps)) return '–';
  if (bps >= 1e9) return `${(bps / 1e9).toFixed(1)} GB/s`;
  if (bps >= 1e6) return `${(bps / 1e6).toFixed(1)} MB/s`;
  if (bps >= 1e3) return `${Math.round(bps / 1e3)} KB/s`;
  return `${Math.max(0, Math.round(bps))} B/s`;
}

export function formatBytes(b: number): string {
  if (b >= 1e9) return `${(b / 1e9).toFixed(1)} GB`;
  if (b >= 1e6) return `${Math.round(b / 1e6)} MB`;
  return `${Math.max(0, Math.round(b / 1e3))} KB`;
}

export type Level = 'ok' | 'warn' | 'high';
export function level(percent: number | null): Level {
  if (percent === null) return 'ok';
  return percent >= 90 ? 'high' : percent >= 70 ? 'warn' : 'ok';
}

export function batteryLabel(b: SysBattery): string {
  const state = b.charging ? 'Charging' : b.plugged ? 'Plugged in' : 'On battery';
  if (!b.charging && !b.plugged && b.minutesLeft !== null && b.minutesLeft > 0) {
    const h = Math.floor(b.minutesLeft / 60);
    const m = b.minutesLeft % 60;
    return `${state} · ${h > 0 ? `${h} h ` : ''}${m} min left`;
  }
  return state;
}

/** Clamp what the native side says to something a bar can draw. */
export function barWidth(percent: number | null): number {
  if (percent === null || !Number.isFinite(percent)) return 0;
  return Math.min(100, Math.max(0, Math.round(percent)));
}
