import { Fragment, useLayoutEffect, useMemo, useRef, type CSSProperties } from 'react';
import type { LyricLine } from '../shared/types';
import { useFrameLoop } from '../island/hooks';
import { headPosition, letterLevels } from './sync';

/**
 * The active lyric line, letter by letter. Letters rise in with a short stagger
 * when the line starts (remount via `key`), then light up left to right as the
 * song is sung. With enhanced-LRC word timing the sweep follows the real words;
 * with plain LRC it is an estimate spread over the line.
 */
export function LetterLine({
  line,
  endMs,
  getPos,
  offsetMs,
  playing,
  speed,
  className,
}: {
  line: LyricLine;
  endMs: number;
  getPos: () => number;
  offsetMs: number;
  playing: boolean;
  speed: number;
  className?: string;
}) {
  const root = useRef<HTMLSpanElement>(null);
  const text = line.text || '♪';
  const words = useMemo(() => text.split(' '), [text]);
  const total = useMemo(() => Array.from(text.replace(/ /g, '')).length, [text]);
  // total stagger stays under ~0.5 s however long the line is
  const step = Math.min(24, 480 / Math.max(1, total));

  const paint = () => {
    const el = root.current;
    if (!el) return;
    const letters = el.querySelectorAll<HTMLElement>('.lf');
    const head = headPosition(line.text, line, endMs, getPos() - offsetMs);
    // `head` counts letters of the line text including spaces; our spans exclude spaces.
    const chars = Array.from(line.text);
    const lit = letterLevels(chars.length, head);
    let k = 0;
    letters.forEach((span) => {
      while (k < chars.length && chars[k] === ' ') k++;
      const l = lit[k] ?? 0;
      k++;
      span.style.setProperty('--l', l.toFixed(3));
      span.style.setProperty('--g', (4 * l * (1 - l)).toFixed(3));
    });
  };

  useFrameLoop(playing, paint);
  useLayoutEffect(paint); // after every render: covers paused, seeking and a fresh line

  let idx = 0;
  return (
    <span
      ref={root}
      className={`letters ${className ?? ''}`}
      style={{ '--speed': speed } as CSSProperties}
      aria-label={text}
      data-testid="letter-line"
    >
      {words.map((w, wi) => (
        <Fragment key={wi}>
        <span className="lw" aria-hidden>
          {Array.from(w).map((c) => {
            const i = idx++;
            return (
              <span key={i} className="lt" style={{ '--d': `${(i * step) / speed}ms` } as CSSProperties}>
                <span className="lf">{c}</span>
              </span>
            );
          })}
        </span>
        {wi < words.length - 1 ? ' ' : ''}
        </Fragment>
      ))}
    </span>
  );
}
