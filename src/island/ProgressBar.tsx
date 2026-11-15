import { useCallback, useEffect, useRef, useState } from 'react';
import { useBackend } from '../backend/hooks';
import type { MediaSession } from '../shared/types';
import { useFrameLoop } from './hooks';

const fmt = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
};

export function ProgressBar({
  session,
  getPos,
  onError,
}: {
  session: MediaSession;
  getPos: () => number;
  onError(msg: string): void;
}) {
  const backend = useBackend();
  const fill = useRef<HTMLDivElement>(null);
  const cur = useRef<HTMLSpanElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const dur = session.durationMs;
  const canSeek = session.capabilities.seek && dur != null && dur > 0;

  const paint = useCallback(
    (pos: number) => {
      const ratio = dur ? Math.min(1, pos / dur) : 0;
      if (fill.current) fill.current.style.transform = `scaleX(${ratio})`;
      if (cur.current) cur.current.textContent = fmt(pos);
    },
    [dur],
  );

  // Live updates while playing; a single paint otherwise (paused, seeked, new track).
  useFrameLoop(session.status === 'playing' && drag === null, () => paint(getPos()));
  useEffect(() => {
    if (drag === null) paint(getPos());
  }, [session, drag, paint, getPos]);

  const ratioAt = (clientX: number) => {
    const r = bar.current!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (clientX - r.left) / r.width));
  };

  return (
    <div className="progress">
      <span ref={cur} className="time" data-testid="time-current">0:00</span>
      <div
        ref={bar}
        className="bar"
        role="slider"
        aria-label="Seek"
        aria-valuemin={0}
        aria-valuemax={dur ?? 0}
        data-seekable={canSeek}
        data-testid="progress-bar"
        onPointerDown={(e) => {
          if (!canSeek) return;
          (e.target as Element).setPointerCapture?.(e.pointerId);
          const r = ratioAt(e.clientX);
          setDrag(r);
          paint(r * dur!);
        }}
        onPointerMove={(e) => {
          if (drag === null) return;
          const r = ratioAt(e.clientX);
          setDrag(r);
          paint(r * dur!);
        }}
        onPointerUp={(e) => {
          if (drag === null) return;
          const r = ratioAt(e.clientX);
          setDrag(null);
          backend
            .mediaTransport({ type: 'seek', positionMs: Math.round(r * dur!) })
            .catch((err) => onError(err instanceof Error ? err.message : String(err)));
        }}
      >
        <div ref={fill} className="fill" />
      </div>
      <span className="time" data-testid="time-total">{dur != null ? fmt(dur) : '--:--'}</span>
    </div>
  );
}
