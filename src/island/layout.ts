import type { AssistantState, Shape } from '../shared/types';

export type Tab = 'music' | 'lyrics' | 'assistant' | 'notifications' | 'clipboard';
export type CompactView = 'idle' | 'media' | 'lyrics' | 'assistant' | 'notification';
export type View = CompactView | 'expanded';

/** Transparent margin the window keeps around the island (sides and bottom). */
export const WINDOW_PAD = 14;

export interface ActivityInput {
  assistant: AssistantState;
  hasSession: boolean;
  lyricsLineVisible: boolean;
  /** A notification just arrived and its preview is showing. */
  notification?: boolean;
}

/**
 * What the collapsed island shows. Priority: an active assistant, then a notification preview
 * (it lasts a few seconds), then lyrics, plain media and finally idle.
 */
export function resolveCompactView(a: ActivityInput): CompactView {
  if (a.assistant !== 'idle') return 'assistant';
  if (a.notification) return 'notification';
  if (a.hasSession && a.lyricsLineVisible) return 'lyrics';
  if (a.hasSession) return 'media';
  return 'idle';
}

const COMPACT_SIZE: Record<CompactView, { w: number; h: number }> = {
  idle: { w: 176, h: 34 },
  media: { w: 248, h: 38 },
  lyrics: { w: 400, h: 42 },
  assistant: { w: 212, h: 42 },
  notification: { w: 360, h: 50 },
};

const EXPANDED_SIZE: Record<Tab, { w: number; h: number }> = {
  music: { w: 540, h: 218 },
  lyrics: { w: 540, h: 262 },
  assistant: { w: 540, h: 340 },
  notifications: { w: 540, h: 320 },
  clipboard: { w: 540, h: 320 },
};

export interface LayoutInput {
  expanded: boolean;
  tab: Tab;
  compact: CompactView;
  sizeScale: number;
  shape: Shape;
  cornerRadius: number;
}

export interface Layout {
  view: View;
  width: number;
  height: number;
  /** Bottom corner radius. */
  radius: number;
  /** Top corner radius (0 for the hanging-notch shape). */
  radiusTop: number;
  /** Design-time size before `sizeScale` (content is laid out at this size, then scaled). */
  baseWidth: number;
  baseHeight: number;
  /** Size of the OS window that must contain the island plus its margin. */
  windowWidth: number;
  windowHeight: number;
}

export function computeLayout(i: LayoutInput): Layout {
  const base = i.expanded ? EXPANDED_SIZE[i.tab] : COMPACT_SIZE[i.compact];
  const width = Math.round(base.w * i.sizeScale);
  const height = Math.round(base.h * i.sizeScale);
  const maxRadius = height / 2;
  const radius = i.expanded ? Math.min(i.cornerRadius, maxRadius) : maxRadius;
  const radiusTop = i.shape === 'notch' ? 0 : radius;
  return {
    view: i.expanded ? 'expanded' : i.compact,
    width,
    height,
    radius,
    radiusTop,
    baseWidth: base.w,
    baseHeight: base.h,
    windowWidth: width + WINDOW_PAD * 2,
    windowHeight: height + WINDOW_PAD,
  };
}
