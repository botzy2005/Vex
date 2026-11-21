import { describe, expect, it } from 'vitest';
import widgets from '../src/shared/widgets.json';
import { computeLayout } from '../src/island/layout';
import { addWidget, dropIndex, initialLayout, MAX_PINNED, MAX_WIDGETS, moveWidget, pinnedCount, removeWidget, resizeWidget, setPinned, SPAN } from '../src/hub/layout';
import { formatRemaining, remainingMs } from '../src/hub/timeFormat';
import type { HubLayout, HubTimer, WidgetDef } from '../src/shared/types';

const def = (id: string, sizes: WidgetDef['sizes'] = ['s', 'm'], defaultSize: WidgetDef['defaultSize'] = 's'): WidgetDef => ({ id, title: id, description: '', sizes, defaultSize });
const layout = (...kinds: string[]): HubLayout => ({ version: 1, widgets: kinds.map((k) => ({ kind: k, size: 's', pinned: false })) });
const order = (l: HubLayout) => l.widgets.map((w) => w.kind);

describe('shipped widget catalog', () => {
  it('has valid, unique ids and default sizes the widget offers', () => {
    const ids = widgets.widgets.map((w) => w.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const w of widgets.widgets) {
      expect(w.id).toMatch(/^[a-z0-9-]{1,32}$/);
      expect(w.sizes).toContain(w.defaultSize);
      for (const s of w.sizes) expect(Object.keys(SPAN)).toContain(s);
    }
  });
  it('starts with just the getting-started card', () => {
    const l = initialLayout(widgets.widgets as WidgetDef[]);
    expect(order(l)).toEqual(['welcome']);
  });
});

describe('layout operations', () => {
  it('adds once, appends, and respects the limit', () => {
    const a = def('a');
    let l = addWidget(layout(), a);
    expect(order(l)).toEqual(['a']);
    expect(addWidget(l, a)).toBe(l);
    const many = layout(...Array.from({ length: MAX_WIDGETS }, (_, i) => `w${i}`));
    expect(addWidget(many, def('extra'))).toBe(many);
    l = addWidget(l, { ...def('b'), defaultSize: 'm' });
    expect(l.widgets[1]).toEqual({ kind: 'b', size: 'm', pinned: false });
  });

  it('removes, and a missing widget is a no-op', () => {
    const l = layout('a', 'b');
    expect(order(removeWidget(l, 'a'))).toEqual(['b']);
    expect(removeWidget(l, 'zzz')).toBe(l);
  });

  it('moves to an index, clamps, and never mutates its input', () => {
    const l = layout('a', 'b', 'c', 'd');
    const frozen = JSON.stringify(l);
    expect(order(moveWidget(l, 'a', 2))).toEqual(['b', 'c', 'a', 'd']);
    expect(order(moveWidget(l, 'd', 0))).toEqual(['d', 'a', 'b', 'c']);
    expect(order(moveWidget(l, 'b', 99))).toEqual(['a', 'c', 'd', 'b']);
    expect(order(moveWidget(l, 'b', -5))).toEqual(['b', 'a', 'c', 'd']);
    expect(moveWidget(l, 'b', 1)).toBe(l);
    expect(moveWidget(l, 'zzz', 0)).toBe(l);
    expect(JSON.stringify(l)).toBe(frozen);
  });

  it('only resizes to sizes the widget offers', () => {
    const a = def('a', ['s', 'l']);
    const l = layout('a');
    expect(resizeWidget(l, 'a', 'l', a).widgets[0].size).toBe('l');
    expect(resizeWidget(l, 'a', 'm', a)).toBe(l);
    expect(resizeWidget(l, 'a', 's', a)).toBe(l);
    expect(resizeWidget(l, 'zzz', 'l', a)).toBe(l);
    expect(resizeWidget(l, 'a', 'l', undefined)).toBe(l);
  });

  it('pins up to the limit and always allows unpinning', () => {
    let l = layout('a', 'b', 'c', 'd', 'e');
    for (const k of ['a', 'b', 'c', 'd']) l = setPinned(l, k, true);
    expect(pinnedCount(l)).toBe(MAX_PINNED);
    expect(setPinned(l, 'e', true)).toBe(l);
    l = setPinned(l, 'a', false);
    expect(pinnedCount(l)).toBe(3);
    expect(pinnedCount(setPinned(l, 'e', true))).toBe(4);
    expect(setPinned(l, 'b', true)).toBe(l);
  });

  it('pinning does not change the order', () => {
    const l = setPinned(layout('a', 'b', 'c'), 'c', true);
    expect(order(l)).toEqual(['a', 'b', 'c']);
  });
});

describe('drag target', () => {
  const rects = [
    { left: 0, top: 0, width: 100, height: 100 },
    { left: 110, top: 0, width: 100, height: 100 },
    { left: 0, top: 110, width: 210, height: 100 },
  ];
  it('picks the card whose centre is nearest the pointer', () => {
    expect(dropIndex(rects, 10, 10)).toBe(0);
    expect(dropIndex(rects, 190, 40)).toBe(1);
    expect(dropIndex(rects, 100, 200)).toBe(2);
    expect(dropIndex(rects, 5000, -5000)).toBe(1);
  });
});

describe('timer display', () => {
  const base = { id: 1, owner: 'x', label: '', durationMs: 60_000 };
  it('mirrors the native remaining-time rule', () => {
    expect(remainingMs({ ...base, state: 'idle' } as HubTimer, 0)).toBe(60_000);
    expect(remainingMs({ ...base, state: 'running', endsAt: 10_000 } as HubTimer, 4_000)).toBe(6_000);
    expect(remainingMs({ ...base, state: 'running', endsAt: 10_000 } as HubTimer, 20_000)).toBe(0);
    expect(remainingMs({ ...base, state: 'paused', remainingMs: 7_000 } as HubTimer, 99)).toBe(7_000);
    expect(remainingMs({ ...base, state: 'finished', at: 1 } as HubTimer, 99)).toBe(0);
  });
  it('formats minutes, hours and days, rounding up', () => {
    expect(formatRemaining(0)).toBe('0:00');
    expect(formatRemaining(1)).toBe('0:01');
    expect(formatRemaining(59_000)).toBe('0:59');
    expect(formatRemaining(25 * 60_000)).toBe('25:00');
    expect(formatRemaining(90 * 60_000)).toBe('1:30:00');
    expect(formatRemaining((3 * 86400 + 5 * 3600) * 1000)).toBe('3d 5h');
    expect(formatRemaining(-5)).toBe('0:00');
  });
});

describe('island layout', () => {
  it('has a size for the widgets tab', () => {
    const l = computeLayout({ expanded: true, tab: 'widgets', compact: 'idle', sizeScale: 1, shape: 'pill', cornerRadius: 26 });
    expect(l.view).toBe('expanded');
    expect(l.width).toBeGreaterThan(400);
  });
});
