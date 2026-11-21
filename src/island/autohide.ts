/** Auto-Hide rules. Pure, so the "when may the island hide" decision is unit-tested without a browser. */

/** After a reveal the island stays up at least this long, so a pointer grazing the edge cannot flicker it. */
export const MIN_VISIBLE_MS = 500;
/** The slide-out animation runs this long before the native window shrinks to the edge strip. */
export const SHRINK_AFTER_MS = 240;

export interface HideInputs {
  mode: 'always' | 'auto';
  /** The pointer is over the island window. */
  pointerInside: boolean;
  expanded: boolean;
  pinned: boolean;
  /** A text field (assistant input) has focus. */
  inputFocused: boolean;
  /** A screenshot confirmation is showing. */
  toast: boolean;
  dragging: boolean;
  /** The assistant is listening, thinking or speaking. */
  assistantBusy: boolean;
}

/** The island may start hiding only in Auto-Hide mode and only when nothing is holding it open. */
export function canHide(i: HideInputs): boolean {
  if (i.mode !== 'auto') return false;
  return !(i.pointerInside || i.expanded || i.pinned || i.inputFocused || i.toast || i.dragging || i.assistantBusy);
}

/** How long to wait before hiding: the configured delay, but never less than the minimum visible time. */
export function hideDelay(delayMs: number, sinceRevealMs: number): number {
  return Math.max(delayMs, MIN_VISIBLE_MS - Math.max(0, sinceRevealMs));
}
