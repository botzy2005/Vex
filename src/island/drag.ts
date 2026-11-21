/** Pure helpers for dragging the island along the top edge. */

/** Pixels the pointer must travel sideways before a press becomes a drag (so a click stays a click). */
export const DRAG_THRESHOLD = 4;

/** The island's new offset (logical px from the screen centre) after the pointer moved from `startX` to `x`. */
export function dragOffset(startOffset: number, startX: number, x: number): number {
  return Math.round(startOffset + (x - startX));
}

export function pastThreshold(startX: number, x: number, threshold = DRAG_THRESHOLD): boolean {
  return Math.abs(x - startX) >= threshold;
}

const TEXT_SELECTOR = 'input:not([type=range]):not([type=checkbox]):not([type=radio]):not([type=button]), textarea, select, [contenteditable=""], [contenteditable="true"]';

/**
 * Alt+drag must never fight text editing: a press inside a text field, text area, select or editable
 * region is left alone (Alt+click there keeps its normal meaning).
 */
export function canStartAltDrag(target: EventTarget | null): boolean {
  return !(target instanceof Element && target.closest(TEXT_SELECTOR));
}
