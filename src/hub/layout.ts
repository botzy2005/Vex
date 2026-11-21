/**
 * Pure dashboard operations. The Hub window applies one of these and saves the result; the native
 * side validates it again. Every function returns a new layout and never mutates its input.
 */
import type { HubLayout, HubPlaced, WidgetDef, WidgetSize } from '../shared/types';

export const MAX_PINNED = 4;
export const MAX_WIDGETS = 24;

/** Grid cells a size occupies (columns x rows). */
export const SPAN: Record<WidgetSize, { cols: number; rows: number }> = {
  s: { cols: 1, rows: 1 },
  m: { cols: 2, rows: 1 },
  t: { cols: 1, rows: 2 },
  l: { cols: 2, rows: 2 },
};

export const SIZE_LABEL: Record<WidgetSize, string> = { s: 'Small', m: 'Wide', t: 'Tall', l: 'Large' };

export function has(layout: HubLayout, kind: string): boolean {
  return layout.widgets.some((w) => w.kind === kind);
}

export function addWidget(layout: HubLayout, def: WidgetDef): HubLayout {
  if (has(layout, def.id) || layout.widgets.length >= MAX_WIDGETS) return layout;
  return { ...layout, widgets: [...layout.widgets, { kind: def.id, size: def.defaultSize, pinned: false }] };
}

export function removeWidget(layout: HubLayout, kind: string): HubLayout {
  if (!has(layout, kind)) return layout;
  return { ...layout, widgets: layout.widgets.filter((w) => w.kind !== kind) };
}

/** Move a widget so it ends up at `toIndex` (clamped) in the resulting order. */
export function moveWidget(layout: HubLayout, kind: string, toIndex: number): HubLayout {
  const from = layout.widgets.findIndex((w) => w.kind === kind);
  if (from < 0) return layout;
  const to = Math.max(0, Math.min(layout.widgets.length - 1, Math.round(toIndex)));
  if (to === from) return layout;
  const widgets = layout.widgets.slice();
  const [item] = widgets.splice(from, 1);
  widgets.splice(to, 0, item);
  return { ...layout, widgets };
}

/** Only sizes the widget offers are accepted. */
export function resizeWidget(layout: HubLayout, kind: string, size: WidgetSize, def: WidgetDef | undefined): HubLayout {
  if (!def || !def.sizes.includes(size)) return layout;
  const w = layout.widgets.find((p) => p.kind === kind);
  if (!w || w.size === size) return layout;
  return { ...layout, widgets: layout.widgets.map((p) => (p.kind === kind ? { ...p, size } : p)) };
}

export function pinnedCount(layout: HubLayout): number {
  return layout.widgets.filter((w) => w.pinned).length;
}

/** Pinning past the limit is refused (the layout comes back unchanged). Unpinning always works. */
export function setPinned(layout: HubLayout, kind: string, pinned: boolean): HubLayout {
  const w = layout.widgets.find((p) => p.kind === kind);
  if (!w || w.pinned === pinned) return layout;
  if (pinned && pinnedCount(layout) >= MAX_PINNED) return layout;
  return { ...layout, widgets: layout.widgets.map((p: HubPlaced) => (p.kind === kind ? { ...p, pinned } : p)) };
}

export function initialLayout(catalog: WidgetDef[]): HubLayout {
  const w = catalog.find((d) => d.id === 'welcome');
  return { version: 1, widgets: w ? [{ kind: w.id, size: w.defaultSize, pinned: false }] : [] };
}

/**
 * Where a dragged card should land. `rects` are the cards' boxes in current order (the dragged card
 * included); the answer is the index of the card whose centre is nearest the pointer.
 */
export function dropIndex(rects: Array<{ left: number; top: number; width: number; height: number }>, x: number, y: number): number {
  let best = 0;
  let bestD = Infinity;
  rects.forEach((r, i) => {
    const dx = x - (r.left + r.width / 2);
    const dy = y - (r.top + r.height / 2);
    const d = dx * dx + dy * dy;
    if (d < bestD) {
      bestD = d;
      best = i;
    }
  });
  return best;
}
