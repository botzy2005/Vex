import type { Settings, SettingsPatch } from './types';

export function hasNull(v: unknown): boolean {
  if (v === null) return true;
  if (v && typeof v === 'object') return Object.values(v).some(hasNull);
  return false;
}

/** RFC 7386 merge for plain JSON objects (used for optimistic UI updates and the test backend). */
export function mergePatch<T>(target: T, patch: unknown): T {
  if (patch === null || typeof patch !== 'object' || Array.isArray(patch)) return patch as T;
  const base: Record<string, unknown> =
    target && typeof target === 'object' && !Array.isArray(target) ? { ...(target as object) } : {};
  for (const [k, v] of Object.entries(patch as Record<string, unknown>)) {
    if (v === null) delete base[k];
    else base[k] = mergePatch(base[k], v);
  }
  return base as T;
}

export const mergeSettings = (s: Settings, p: SettingsPatch): Settings => mergePatch(s, p);
