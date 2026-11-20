import { useEffect, useRef, useState } from 'react';

/**
 * The album cover as a soft colour field behind the expanded player.
 * The cover is drawn once into a tiny canvas that CSS stretches (smooth upscaling is the blur), so there is no
 * blur() filter to re-render while the island animates its size. A dark scrim keeps text readable on any cover.
 */
export function MediaBackdrop({ url, on }: { url: string; on: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let alive = true;
    setReady(false);
    const img = new Image();
    img.onload = () => {
      const c = canvas.current;
      const g = c?.getContext('2d');
      if (!alive || !c || !g) return;
      // Two steps (cover -> 64px -> 24px) average the pixels instead of sampling a few of them.
      const mid = document.createElement('canvas');
      mid.width = mid.height = 64;
      const mg = mid.getContext('2d');
      if (!mg) return;
      mg.imageSmoothingQuality = 'high';
      mg.drawImage(img, 0, 0, 64, 64);
      g.imageSmoothingEnabled = true;
      g.imageSmoothingQuality = 'high';
      g.clearRect(0, 0, c.width, c.height);
      g.drawImage(mid, 0, 0, c.width, c.height);
      setReady(true);
    };
    img.onerror = () => {};
    img.src = url;
    return () => {
      alive = false;
      img.onload = null;
    };
  }, [url]);

  return (
    <div className="media-bg" aria-hidden data-on={on && ready} data-testid="media-backdrop">
      <canvas ref={canvas} width={24} height={24} />
      <div className="media-scrim" />
    </div>
  );
}
