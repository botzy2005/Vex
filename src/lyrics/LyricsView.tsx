import { useLayoutEffect, useRef, type CSSProperties } from 'react';
import type { LyricsHook } from './useLyrics';
import type { MediaSession, Settings } from '../shared/types';
import { useFrameLoop } from '../island/hooks';
import { LetterLine } from './LetterLine';
import { lineEndMs, wordFractions } from './sync';

export function LyricsView({
  lyrics,
  session,
  activeIndex,
  getPos,
  music,
  onSeek,
}: {
  lyrics: LyricsHook;
  session: MediaSession | null;
  activeIndex: number;
  getPos: () => number;
  music: Settings['music'];
  /** Jump playback to a position in milliseconds (click a line). */
  onSeek(positionMs: number): void;
}) {
  const { doc } = lyrics;
  const viewport = useRef<HTMLDivElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const [offset, speed] = [music.lyricsOffsetMs, music.lyricsAnimationSpeed];

  // Keep the active line vertically centred; the CSS transition makes it a smooth scroll.
  useLayoutEffect(() => {
    const vp = viewport.current;
    const tr = track.current;
    if (!vp || !tr || !doc) return;
    const el = tr.children[Math.max(0, activeIndex)] as HTMLElement | undefined;
    if (!el) return;
    tr.style.transform = `translateY(${vp.clientHeight / 2 - (el.offsetTop + el.offsetHeight / 2)}px)`;
  }, [activeIndex, doc, music.lyricsFontSize]);

  // Karaoke: fill the words of the active line according to the live position.
  const line = doc && activeIndex >= 0 ? doc.lines[activeIndex] : null;
  const letters = music.lyricsStyle === 'letters';
  const karaoke = !letters && music.karaoke && !!line?.words;
  const paintWords = () => {
    if (!doc || !line?.words || !track.current) return;
    const spans = track.current.querySelectorAll<HTMLElement>('[data-active="true"] .kw');
    const f = wordFractions(line, lineEndMs(doc.lines, activeIndex), getPos() - offset);
    spans.forEach((s, i) => s.style.setProperty('--p', `${((f[i] ?? 0) * 100).toFixed(1)}%`));
  };
  useFrameLoop(karaoke && session?.status === 'playing', paintWords);
  useLayoutEffect(() => {
    if (karaoke) paintWords();
  }); // paint once after every render (covers paused / seek)

  const onFile = async (file: File | undefined) => {
    if (!file) return;
    lyrics.loadFromText(await file.text());
    if (fileInput.current) fileInput.current.value = '';
  };

  if (!session) return <div className="empty" data-testid="lyrics-empty"><strong>No lyrics yet</strong><span>Lyrics appear here while a song is playing.</span></div>;

  if (!doc) {
    const loadBtn = (
      <>
        <button className="pill-btn" onClick={() => fileInput.current?.click()} data-testid="load-lrc">Load .lrc file…</button>
        <input ref={fileInput} type="file" accept=".lrc,.txt,text/plain" hidden data-testid="lrc-input" onChange={(e) => onFile(e.target.files?.[0])} />
        {lyrics.error && <span className="inline-error" role="alert">{lyrics.error}</span>}
      </>
    );
    if (lyrics.status === 'plain' && lyrics.plain) {
      return (
        <div className="lyrics-view" data-testid="lyrics-plain">
          <div className="plain-note">Lyrics found, but without timing, so they can't follow the song. {lyrics.source}</div>
          <div className="plain-text" style={{ fontSize: music.lyricsFontSize - 2, fontFamily: music.lyricsFont }}>{lyrics.plain}</div>
          <div className="lyr-foot"><span>Have a synced version?</span>{loadBtn}</div>
        </div>
      );
    }
    return (
      <div className="empty" data-testid="lyrics-empty" data-status={lyrics.status}>
        {lyrics.status === 'loading' && <strong>Looking for lyrics…</strong>}
        {lyrics.status === 'instrumental' && <><strong>Instrumental ♪</strong><span>This track has no lyrics.</span></>}
        {lyrics.status === 'error' && (
          <>
            <strong>Couldn't reach the lyrics service</strong>
            <span>Check your connection. VEX will try again shortly.</span>
            <button className="pill-btn" onClick={lyrics.retry} data-testid="retry-lyrics">Try again</button>
            {loadBtn}
          </>
        )}
        {lyrics.status === 'not-found' && (
          <>
            <strong>No synced lyrics found</strong>
            <span>
              {lyrics.online
                ? 'LRCLIB has no timed lyrics for this track. You can load your own .lrc file.'
                : 'Online lookup is off (Settings → Music & lyrics). You can load a .lrc file for this track.'}
            </span>
            {loadBtn}
          </>
        )}
      </div>
    );
  }

  return (
    <div
      className="lyrics-view"
      style={{ '--lyric-size': `${music.lyricsFontSize}px`, '--speed': speed, fontFamily: music.lyricsFont } as CSSProperties}
      data-testid="lyrics-view"
    >
      <div className="lyr-viewport" ref={viewport}>
        <div className="lyr-track" ref={track}>
          {doc.lines.map((l, i) => {
            const active = i === activeIndex;
            const dist = Math.min(3, Math.abs(i - activeIndex));
            return (
              <div
                key={i}
                className="lyr-line"
                data-active={active}
                data-dist={activeIndex < 0 ? Math.min(3, i) : dist}
                data-testid={active ? 'lyric-active' : undefined}
                data-seekable={session.capabilities.seek}
                onClick={() => session.capabilities.seek && onSeek(l.timeMs + offset)}
              >
                {active && letters ? (
                  <LetterLine key={`a${i}`} line={l} endMs={lineEndMs(doc.lines, i)} getPos={getPos} offsetMs={offset} playing={session.status === 'playing'} speed={speed} />
                ) : active && karaoke && l.words ? (
                  l.words.map((w, wi) => <span key={wi} className="kw">{w.text}</span>)
                ) : (
                  l.text || '♪'
                )}
              </div>
            );
          })}
        </div>
      </div>
      <div className="lyr-foot">
        <span>{doc.source === 'LRCLIB' ? 'Lyrics from LRCLIB' : doc.source}</span>
        <button className="link-btn" onClick={lyrics.clear} data-testid="clear-lrc">Remove</button>
      </div>
    </div>
  );

}
