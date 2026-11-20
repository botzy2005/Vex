// Undo and redo as a plain value, independent of what is being edited.

export interface History<T> {
  past: T[];
  present: T;
  future: T[];
}

/** How many steps can be undone. Older steps are dropped so a long session cannot grow without bound. */
export const HISTORY_CAP = 60;

export const createHistory = <T>(present: T): History<T> => ({ past: [], present, future: [] });

export function push<T>(h: History<T>, next: T, cap = HISTORY_CAP): History<T> {
  const past = [...h.past, h.present];
  while (past.length > cap) past.shift();
  return { past, present: next, future: [] }; // a new action ends the redo chain
}

export const canUndo = <T>(h: History<T>) => h.past.length > 0;
export const canRedo = <T>(h: History<T>) => h.future.length > 0;

export function undo<T>(h: History<T>): History<T> {
  if (!canUndo(h)) return h;
  return { past: h.past.slice(0, -1), present: h.past[h.past.length - 1], future: [h.present, ...h.future] };
}

export function redo<T>(h: History<T>): History<T> {
  if (!canRedo(h)) return h;
  return { past: [...h.past, h.present], present: h.future[0], future: h.future.slice(1) };
}
