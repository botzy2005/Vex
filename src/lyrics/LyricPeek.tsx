import { useMemo, useRef, type CSSProperties } from 'react';
import type { Settings } from '../shared/types';
import { fitStage, peekCap } from './fit';
import { canvasMeasure, familyOf, useElementSize } from './useTextFit';
import { LetterLine } from './LetterLine';
import { lineEndMs } from './sync';
import type { LyricsHook } from './useLyrics';

/** The current (and next) lyric line shown inside the player, so lyrics sit beside the controls. */
export function LyricPeek({
  lyrics,
  activeIndex,
  getPos,
  music,
  playing,
}: {
  lyrics: LyricsHook;
  activeIndex: number;
  getPos: () => number;
  music: Settings['music'];
  playing: boolean;
}) {
  const { doc } = lyrics;
  const box = useRef<HTMLDivElement>(null);
  const { width, height } = useElementSize(box);
  const cur = doc && activeIndex >= 0 ? doc.lines[activeIndex] : null;
  const nowText = cur ? cur.text || '♪' : '';
  const hasPrev = activeIndex > 0;
  const hasNext = !!doc && activeIndex >= 0 && activeIndex + 1 < doc.lines.length;
  const family = music.lyricsFont || familyOf(box.current);
  const sizes = useMemo(() => {
    if (!nowText || width < 40 || height < 30) return null;
    return fitStage({ width: width - 4, height, now: nowText, hasPrev, hasNext, cap: peekCap(music.lyricsFontSize), measure: canvasMeasure(family || 'sans-serif', 700), breakWords: music.lyricsStyle !== 'letters' });
  }, [nowText, width, height, hasPrev, hasNext, music.lyricsFontSize, family]);
  const vars = sizes ? ({ '--peek-now': `${sizes.now}px`, '--peek-side': `${sizes.side}px` } as CSSProperties) : undefined;
  let body: React.ReactNode = null;
  if (!music.lyricsEnabled) return null;
  if (doc && activeIndex >= 0) {
    const l = doc.lines[activeIndex];
    const prev = activeIndex > 0 ? doc.lines[activeIndex - 1] : undefined;
    const next = doc.lines[activeIndex + 1];
    body = (
      <>
        <div className="peek-prev" style={{ fontFamily: music.lyricsFont }} data-testid="peek-prev">{prev ? prev.text || '♪' : '\u00a0'}</div>
        <div className="peek-now" style={{ fontFamily: music.lyricsFont }} data-testid="peek-now">
          {music.lyricsStyle === 'letters' ? (
            <LetterLine key={`p${activeIndex}`} line={l} endMs={lineEndMs(doc.lines, activeIndex)} getPos={getPos} offsetMs={music.lyricsOffsetMs} playing={playing} speed={music.lyricsAnimationSpeed} />
          ) : (
            <span key={`p${activeIndex}`} className="peek-fade">{l.text || '♪'}</span>
          )}
        </div>
        {next && <div className="peek-next" style={{ fontFamily: music.lyricsFont }}>{next.text || '♪'}</div>}
      </>
    );
  } else if (doc) {
    body = <div className="peek-hint">♪</div>;
  } else if (lyrics.status === 'loading') {
    body = <div className="peek-hint">Looking for lyrics…</div>;
  } else if (lyrics.status === 'instrumental') {
    body = <div className="peek-hint">Instrumental ♪</div>;
  } else if (lyrics.status === 'error') {
    body = <div className="peek-hint">Lyrics unavailable (offline?)</div>;
  } else if (lyrics.status === 'not-found' || lyrics.status === 'plain') {
    body = <div className="peek-hint">No synced lyrics</div>;
  }
  return <div className="peek" data-testid="lyric-peek" ref={box} style={vars}>{body}</div>;
}
