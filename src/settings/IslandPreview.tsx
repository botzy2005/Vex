import type { CSSProperties } from 'react';
import { onAccent } from '../island/accent';
import type { Settings } from '../shared/types';

/** Mirrors the surface rule of the real island: never less than ~90% opaque. */
export const effectiveGlass = (glass: number) => 0.82 + 0.18 * glass;

/**
 * A small, static picture of the island that follows the appearance settings as you change them.
 * It is illustrative (fixed sample track), and does not play or read anything.
 */
export function IslandPreview({ a, theme, accent }: { a: Settings['appearance']; theme: 'dark' | 'light'; accent: string }) {
  const r = a.cornerRadius;
  const radius = a.shape === 'notch' ? `0 0 ${r}px ${r}px` : `${r}px`;
  const style = {
    '--accent': accent,
    '--on-accent': onAccent(accent),
    '--glass': a.glassOpacity,
  } as CSSProperties;
  return (
    <div className="vex-preview" data-theme={theme} style={style} data-testid="island-preview" data-shape={a.shape} aria-label="Preview of the island with these settings" role="img">
      <div className="pv-desk" aria-hidden style={{ height: Math.round(58 + 170 * a.sizeScale) }}>
        <i /><i /><i /><i />
        <div className="pv-scale" style={{ transform: `scale(${a.sizeScale})` }}>
          <div className="pv-pill" style={{ borderRadius: a.shape === 'notch' ? '0 0 16px 16px' : 16 }} data-testid="preview-pill">
            <span className="pv-art sm" />
            <b>Night Drive</b>
            <span className="pv-eq"><i /><i /><i /></span>
          </div>
          <div className="pv-island" style={{ borderRadius: radius }} data-testid="preview-island">
            <div className="pv-tabs"><span className="on" /><span /><span /><span /></div>
            <div className="pv-main">
              <span className="pv-art" />
              <div className="pv-text">
                <b>Night Drive</b>
                <em>The Testers</em>
                <div className="pv-lyrics"><s>Streetlights drifting past</s><strong>Counting down the miles</strong><s>Every window, a song</s></div>
              </div>
            </div>
            <div className="pv-bar"><i /></div>
            <div className="pv-ctl"><span /><span className="play" /><span /></div>
          </div>
        </div>
      </div>
    </div>
  );
}
