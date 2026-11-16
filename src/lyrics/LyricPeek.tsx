import type { Settings } from '../shared/types';
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
  let body: React.ReactNode = null;
  if (!music.lyricsEnabled) return null;
  if (doc && activeIndex >= 0) {
    const l = doc.lines[activeIndex];
    const next = doc.lines[activeIndex + 1];
    body = (
      <>
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
  return <div className="peek" data-testid="lyric-peek">{body}</div>;
}
