import { useEffect, useRef, useState } from 'react';
import { useBackend } from '../backend/hooks';
import { MIN_DRAG, normBox, type Box } from './model';

/**
 * The selection layer shown over one display while choosing a region. The picture itself was frozen
 * natively when the shortcut was pressed, so this page only reports where the user dragged.
 */
export function Overlay() {
  const backend = useBackend();
  const monitor = Number(new URLSearchParams(location.search).get('i') ?? '0') || 0;
  const [box, setBox] = useState<Box | null>(null);
  const [hint, setHint] = useState<string | null>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  const busy = useRef(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') void backend.screenshotRegionCancel();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [backend]);

  const dpr = window.devicePixelRatio || 1;

  const finish = async (b: Box, a: { x: number; y: number }, z: { x: number; y: number }) => {
    if (b.w < MIN_DRAG || b.h < MIN_DRAG) {
      setBox(null);
      setHint('Drag to draw a rectangle');
      return;
    }
    busy.current = true;
    try {
      await backend.screenshotRegionSelect(monitor, a.x, a.y, z.x, z.y);
    } catch (e) {
      busy.current = false;
      setBox(null);
      setHint(String(e instanceof Error ? e.message : e));
    }
  };

  return (
    <div
      className="ov"
      data-testid="overlay"
      data-monitor={monitor}
      data-selecting={!!box}
      onPointerDown={(e) => {
        if (e.button === 2) return void backend.screenshotRegionCancel();
        if (e.button !== 0 || busy.current) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        start.current = { x: e.clientX, y: e.clientY };
        setHint(null);
        setBox({ x: e.clientX, y: e.clientY, w: 0, h: 0 });
      }}
      onPointerMove={(e) => {
        if (start.current) setBox(normBox(start.current, { x: e.clientX, y: e.clientY }));
      }}
      onPointerUp={(e) => {
        const a = start.current;
        start.current = null;
        if (!a || busy.current) return;
        const z = { x: e.clientX, y: e.clientY };
        void finish(normBox(a, z), a, z);
      }}
      onContextMenu={(e) => e.preventDefault()}
    >
      {!box && (
        <div className="ov-hint" data-testid="overlay-hint">
          {hint ?? 'Drag to select an area  ·  Esc to cancel'}
        </div>
      )}
      {box && (
        <>
          <div className="ov-sel" data-testid="overlay-selection" style={{ left: box.x, top: box.y, width: box.w, height: box.h }} />
          <div className="ov-size" data-testid="overlay-size" style={{ left: box.x, top: Math.max(0, box.y - 26) }}>
            {Math.round(box.w * dpr)} × {Math.round(box.h * dpr)}
          </div>
        </>
      )}
    </div>
  );
}
