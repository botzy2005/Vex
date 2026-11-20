import type { Settings } from '../shared/types';

type Look = Pick<Settings['appearance'], 'theme' | 'shape' | 'glassOpacity' | 'cornerRadius' | 'accentSource' | 'customAccent'>;

export interface Preset {
  id: string;
  name: string;
  blurb: string;
  look: Look;
}

/** Ready-made looks. They only set existing appearance settings, so anything can still be fine-tuned afterwards. */
export const PRESETS: Preset[] = [
  { id: 'midnight', name: 'Midnight', blurb: 'The VEX default: deep dark, indigo accent, colour from the album cover.', look: { theme: 'dark', shape: 'pill', glassOpacity: 0.92, cornerRadius: 26, accentSource: 'artwork', customAccent: '#a5b4fc' } },
  { id: 'graphite', name: 'Graphite', blurb: 'Neutral and quiet. Almost opaque, soft grey accent.', look: { theme: 'dark', shape: 'pill', glassOpacity: 1, cornerRadius: 22, accentSource: 'custom', customAccent: '#c3c8d4' } },
  { id: 'aurora', name: 'Aurora', blurb: 'Cool teal on dark with rounder corners.', look: { theme: 'dark', shape: 'pill', glassOpacity: 0.92, cornerRadius: 32, accentSource: 'custom', customAccent: '#5eead4' } },
  { id: 'ember', name: 'Ember', blurb: 'Warm orange accent on dark.', look: { theme: 'dark', shape: 'pill', glassOpacity: 0.94, cornerRadius: 24, accentSource: 'custom', customAccent: '#fb923c' } },
  { id: 'notch', name: 'Notch', blurb: 'Hangs flat from the top edge, like a hardware notch.', look: { theme: 'dark', shape: 'notch', glassOpacity: 0.96, cornerRadius: 20, accentSource: 'artwork', customAccent: '#a5b4fc' } },
  { id: 'daylight', name: 'Daylight', blurb: 'Light surface for bright desktops.', look: { theme: 'light', shape: 'pill', glassOpacity: 0.96, cornerRadius: 26, accentSource: 'custom', customAccent: '#5b6cd6' } },
];

/** The preset the current appearance matches exactly, if any. */
export function activePreset(a: Settings['appearance']): string | null {
  const hit = PRESETS.find(
    (p) =>
      p.look.theme === a.theme &&
      p.look.shape === a.shape &&
      Math.abs(p.look.glassOpacity - a.glassOpacity) < 0.005 &&
      p.look.cornerRadius === a.cornerRadius &&
      p.look.accentSource === a.accentSource &&
      p.look.customAccent.toLowerCase() === a.customAccent.toLowerCase(),
  );
  return hit?.id ?? null;
}
