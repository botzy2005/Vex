import { useCallback, useEffect, useRef, useState } from 'react';
import type { Backend } from '../backend/backend';
import { canHide, hideDelay, SHRINK_AFTER_MS, type HideInputs } from './autohide';

export type HidePhase = 'visible' | 'hiding' | 'hidden';

/**
 * Auto-Hide state machine. Timers exist only while a hide is pending or animating: in Always Visible mode, or
 * while the island is held open, nothing is scheduled at all.
 *
 *   visible -> (idle for the delay) -> hiding (slide out) -> hidden (native window shrinks to the edge strip)
 *   hidden / hiding -> reveal() -> visible
 */
export function useAutoHide(backend: Backend, delayMs: number, holds: Omit<HideInputs, 'pointerInside'>) {
  const [phase, setPhase] = useState<HidePhase>('visible');
  const [inside, setInside] = useState(false);
  const phaseRef = useRef<HidePhase>('visible');
  const revealedAt = useRef(Date.now());
  const hideTimer = useRef<number | undefined>(undefined);
  const shrinkTimer = useRef<number | undefined>(undefined);
  const set = (p: HidePhase) => {
    phaseRef.current = p;
    setPhase(p);
  };
  const clearTimers = () => {
    window.clearTimeout(hideTimer.current);
    window.clearTimeout(shrinkTimer.current);
  };

  /** `pointerHere`: the reveal came from the pointer (edge strip), so it is known to be over the island. */
  const reveal = useCallback((pointerHere = false) => {
    clearTimers();
    revealedAt.current = Date.now();
    if (pointerHere) {
      setInside(true);
      // The island appears under a resting pointer, so the browser may never send it an enter/leave pair.
      // Re-sync from the first real movement afterwards.
      const resync = (e: PointerEvent) => setInside(e.target instanceof Element && !!e.target.closest('.island'));
      // Wait for the slide-in to finish first, so the island can be hit-tested.
      window.setTimeout(() => window.addEventListener('pointermove', resync, { once: true, capture: true }), 300);
    }
    if (phaseRef.current === 'visible') return;
    const wasHidden = phaseRef.current === 'hidden';
    if (!wasHidden) return set('visible');
    // Grow the native window first so the slide-in has room, then animate.
    backend
      .setIslandHidden(false)
      .catch(() => {})
      .finally(() => set('visible'));
  }, [backend]);

  const mode = holds.mode;
  const idle = canHide({ ...holds, pointerInside: inside });

  useEffect(() => {
    if (!idle) {
      // Something is holding the island open: cancel a pending hide and bring back a half-hidden island.
      if (phaseRef.current === 'hiding') {
        clearTimers();
        set('visible');
      }
      return;
    }
    if (phaseRef.current !== 'visible') return;
    const wait = hideDelay(delayMs, Date.now() - revealedAt.current);
    hideTimer.current = window.setTimeout(() => {
      set('hiding');
      shrinkTimer.current = window.setTimeout(() => {
        backend
          .setIslandHidden(true)
          .then((applied) => set(applied ? 'hidden' : 'visible'))
          .catch(() => set('visible'));
      }, SHRINK_AFTER_MS);
    }, wait);
    // Only the pending hide is cancelled here; once the slide-out has started it runs to its end.
    return () => window.clearTimeout(hideTimer.current);
  }, [idle, delayMs, backend, phase]);

  // Leaving Auto-Hide mode (e.g. from Settings) brings the island back.
  useEffect(() => {
    if (mode !== 'auto' && phaseRef.current !== 'visible') {
      clearTimers();
      set('visible');
    }
  }, [mode]);

  useEffect(() => clearTimers, []);

  return {
    phase,
    reveal,
    onPointerEnter: () => setInside(true),
    onPointerLeave: () => setInside(false),
  };
}
