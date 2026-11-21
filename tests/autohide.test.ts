import { describe, expect, it } from 'vitest';
import { canHide, hideDelay, MIN_VISIBLE_MS, type HideInputs } from '../src/island/autohide';

const idle: HideInputs = { mode: 'auto', pointerInside: false, expanded: false, pinned: false, inputFocused: false, toast: false, dragging: false, assistantBusy: false };

describe('auto-hide rules', () => {
  it('never hides in Always Visible mode', () => {
    expect(canHide({ ...idle, mode: 'always' })).toBe(false);
  });
  it('hides an idle, collapsed island in Auto-Hide mode', () => {
    expect(canHide(idle)).toBe(true);
  });
  it.each([
    ['pointer inside', { pointerInside: true }],
    ['expanded', { expanded: true }],
    ['pinned', { pinned: true }],
    ['text field focused', { inputFocused: true }],
    ['screenshot toast', { toast: true }],
    ['dragging', { dragging: true }],
    ['assistant busy', { assistantBusy: true }],
  ])('stays visible while: %s', (_n, hold) => {
    expect(canHide({ ...idle, ...hold })).toBe(false);
  });
  it('waits the configured delay, but never less than the minimum visible time after a reveal', () => {
    expect(hideDelay(800, 10_000)).toBe(800);
    expect(hideDelay(200, 0)).toBe(MIN_VISIBLE_MS);
    expect(hideDelay(200, 300)).toBe(MIN_VISIBLE_MS - 300 > 200 ? MIN_VISIBLE_MS - 300 : 200);
    expect(hideDelay(200, -50)).toBe(MIN_VISIBLE_MS);
  });
});
