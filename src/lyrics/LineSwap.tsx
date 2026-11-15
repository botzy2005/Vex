import { useEffect, useRef, useState, type CSSProperties } from 'react';

/** Single-line text that slides the old line out and the new one in whenever `text` changes. */
export function LineSwap({ text, speed = 1 }: { text: string; speed?: number }) {
  const [prev, setPrev] = useState<{ text: string; n: number } | null>(null);
  const last = useRef(text);
  const counter = useRef(0);

  useEffect(() => {
    if (last.current === text) return;
    counter.current += 1;
    setPrev({ text: last.current, n: counter.current });
    last.current = text;
    const id = window.setTimeout(() => setPrev(null), 420 / speed);
    return () => window.clearTimeout(id);
  }, [text, speed]);

  return (
    <div className="swap" style={{ '--speed': speed } as CSSProperties} data-testid="compact-lyric">
      {prev && <span key={`o${prev.n}`} className="swap-out" aria-hidden>{prev.text || '♪'}</span>}
      <span key={`i${counter.current}`} className="swap-in" data-testid="compact-lyric-text">{text || '♪'}</span>
    </div>
  );
}
