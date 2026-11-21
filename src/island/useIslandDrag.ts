import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import type { Backend } from '../backend/backend';
import { canStartAltDrag, dragOffset, pastThreshold } from './drag';

interface Session {
  startX: number;
  startOffset: number;
  pointerId: number;
  el: HTMLElement;
  active: boolean;
  last: number;
}

/**
 * Repositioning along the top edge. A drag starts from the grip, or from Alt+press anywhere that is not a
 * text field, and only once the pointer has moved a few pixels, so plain clicks (even Alt+clicks) are
 * unaffected. While dragging, the native side moves the window (and clamps it to the monitor); the position
 * is saved once, on release. Escape puts it back where it was. A click that ends a drag is swallowed.
 */
export function useIslandDrag(backend: Backend, offsetX: number) {
  const [dragging, setDragging] = useState(false);
  const offsetRef = useRef(offsetX);
  offsetRef.current = offsetX;
  const session = useRef<Session | null>(null);
  const raf = useRef(0);
  const pending = useRef<number | null>(null);
  const inflight = useRef(false);
  const cleanup = useRef<(() => void) | null>(null);

  const send = useCallback(() => {
    raf.current = 0;
    if (inflight.current || pending.current === null) return;
    const v = pending.current;
    pending.current = null;
    inflight.current = true;
    backend
      .setIslandOffset(v, false)
      .catch(() => {})
      .finally(() => {
        inflight.current = false;
        if (pending.current !== null && !raf.current) raf.current = requestAnimationFrame(send);
      });
  }, [backend]);

  const begin = useCallback(
    (e: ReactPointerEvent, el: HTMLElement | null) => {
      if (session.current || !el || e.button !== 0) return;
      const s: Session = { startX: e.screenX, startOffset: offsetRef.current, pointerId: e.pointerId, el, active: false, last: offsetRef.current };
      session.current = s;

      const swallowClick = (ev: MouseEvent) => {
        ev.preventDefault();
        ev.stopPropagation();
      };
      const finish = (commitTo: number | null) => {
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
        window.removeEventListener('pointercancel', cancel);
        window.removeEventListener('keydown', key, true);
        cleanup.current = null;
        session.current = null;
        if (!s.active) return;
        window.cancelAnimationFrame(raf.current);
        raf.current = 0;
        pending.current = null;
        try {
          s.el.releasePointerCapture(s.pointerId);
        } catch {
          /* already released */
        }
        setDragging(false);
        // The click that follows a drag must not press whatever is under the pointer.
        window.addEventListener('click', swallowClick, { capture: true, once: true });
        window.setTimeout(() => window.removeEventListener('click', swallowClick, true), 60);
        // Wait for any in-flight move so the final, saved position is the last one the native side sees.
        void backend.setIslandOffset(commitTo ?? s.startOffset, true).catch(() => {});
      };
      const move = (ev: PointerEvent) => {
        if (ev.pointerId !== s.pointerId) return;
        if (!s.active) {
          if (!pastThreshold(s.startX, ev.screenX)) return;
          s.active = true;
          setDragging(true);
          try {
            s.el.setPointerCapture(s.pointerId);
          } catch {
            /* the window-level listeners still track it */
          }
        }
        s.last = dragOffset(s.startOffset, s.startX, ev.screenX);
        pending.current = s.last;
        if (!raf.current) raf.current = requestAnimationFrame(send);
      };
      const up = (ev: PointerEvent) => {
        if (ev.pointerId === s.pointerId) finish(s.last);
      };
      const cancel = (ev: PointerEvent) => {
        if (ev.pointerId === s.pointerId) finish(null);
      };
      const key = (ev: KeyboardEvent) => {
        if (ev.key === 'Escape' && s.active) {
          ev.stopPropagation();
          finish(null);
        }
      };
      cleanup.current = () => finish(null);
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
      window.addEventListener('pointercancel', cancel);
      window.addEventListener('keydown', key, true);
    },
    [backend, send],
  );

  useEffect(() => () => cleanup.current?.(), []);

  /** Props for the island element: Alt+press starts a drag (never inside a text field). */
  const onIslandPointerDown = (e: ReactPointerEvent<HTMLElement>) => {
    if (e.altKey && canStartAltDrag(e.target)) begin(e, e.currentTarget);
  };
  /** Props for the grip: a plain press starts a drag. */
  const onGripPointerDown = (el: HTMLElement | null) => (e: ReactPointerEvent) => {
    e.stopPropagation();
    begin(e, el);
  };

  return { dragging, onIslandPointerDown, onGripPointerDown };
}
