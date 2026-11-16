import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { AssistantFace } from '../assistant/AssistantFace';
import { AssistantPanel } from '../assistant/AssistantPanel';
import { useAssistant, useBackend, useMedia, useResolvedTheme, useSettings, useSubscription } from '../backend/hooks';
import { LetterLine } from '../lyrics/LetterLine';
import { LineSwap } from '../lyrics/LineSwap';
import { lineEndMs } from '../lyrics/sync';
import { LyricPeek } from '../lyrics/LyricPeek';
import { LyricsView } from '../lyrics/LyricsView';
import { useLyrics } from '../lyrics/useLyrics';
import { GearIcon, LyricsIcon, MusicIcon, PinIcon, PowerIcon, SparkIcon } from './icons';
import { useAccent, useActiveLine, useExpansion, useIslandSpring, usePlaybackPosition, useWindowSize } from './hooks';
import { computeLayout, resolveCompactView, type Tab } from './layout';
import { Artwork, MusicPanel } from './MusicPanel';
import { Visualizer } from './Visualizer';

const TABS: Array<{ id: Tab; label: string; icon: typeof MusicIcon }> = [
  { id: 'music', label: 'Music', icon: MusicIcon },
  { id: 'lyrics', label: 'Lyrics', icon: LyricsIcon },
  { id: 'assistant', label: 'Assistant', icon: SparkIcon },
];

const ASSISTANT_LABEL = { idle: '', listening: 'Listening', thinking: 'Thinking', speaking: 'Speaking', error: 'Error' } as const;

export function IslandApp() {
  const backend = useBackend();
  const { settings } = useSettings();
  const media = useMedia();
  const assistant = useAssistant();
  const session = media.session;

  const [tab, setTab] = useState<Tab>('music');
  const [inputFocused, setInputFocused] = useState(false);
  const exp = useExpansion(settings?.island, inputFocused);

  const getPos = usePlaybackPosition(session);
  const music = settings?.music;
  const lyrics = useLyrics(session, backend, music?.lyricsOnline ?? true);
  const lyricsOffset = music?.lyricsOffsetMs ?? 0;
  const activeIndex = useActiveLine(music?.lyricsEnabled ? lyrics.doc : null, getPos, lyricsOffset, session);
  const activeText = lyrics.doc && activeIndex >= 0 ? lyrics.doc.lines[activeIndex].text : '';
  const accent = useAccent(settings, session?.artworkUrl);
  const theme = useResolvedTheme(settings?.appearance.theme);

  useSubscription<null>((cb) => backend.onIslandToggle(() => cb(null)), () => exp.toggleFromHotkey());

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && exp.expanded && exp.collapse();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const a = settings?.appearance;
  const compact = resolveCompactView({
    assistant: assistant.state,
    hasSession: !!session,
    lyricsLineVisible: !!music?.lyricsEnabled && !!music?.lyricsInCollapsed && lyrics.doc !== null && activeIndex >= 0,
  });
  const layout = computeLayout({
    expanded: exp.expanded,
    tab,
    compact,
    sizeScale: a?.sizeScale ?? 1,
    shape: a?.shape ?? 'pill',
    cornerRadius: a?.cornerRadius ?? 26,
  });

  const islandRef = useRef<HTMLDivElement>(null);
  useIslandSpring(islandRef, layout, a?.motion ?? 'normal');
  useWindowSize(backend, layout);

  if (!settings || !a || !music) return null;

  const seek = (positionMs: number) => void backend.mediaTransport({ type: 'seek', positionMs }).catch(() => {});
  const playing = session?.status === 'playing';

  return (
    <div
      className="stage"
      data-theme={theme}
      data-motion={a.motion}
      style={{ '--accent': accent, '--glass': a.glassOpacity, '--speed': music.lyricsAnimationSpeed } as CSSProperties}
    >
      <div
        ref={islandRef}
        className="island"
        data-testid="island"
        data-view={layout.view}
        data-expanded={exp.expanded}
        data-pinned={exp.pinned}
        data-shape={a.shape}
        data-assistant={assistant.state}
        onPointerEnter={exp.onPointerEnter}
        onPointerLeave={exp.onPointerLeave}
        onClick={() => !exp.expanded && exp.expand()}
      >
        <div
          className="content"
          style={{
            width: layout.baseWidth,
            height: layout.baseHeight,
            transform: `translateX(-50%) scale(${a.sizeScale})`,
          }}
        >
          {layout.view === 'idle' && <div className="v idle" key="idle" data-testid="view-idle" />}

          {layout.view === 'media' && session && (
            <div className="v compact" key="media" data-testid="view-media">
              {music.artwork && <Artwork session={session} className="mini" />}
              <div className="grow compact-title">{session.title}</div>
              {music.visualizer && <Visualizer playing={playing} />}
            </div>
          )}

          {layout.view === 'lyrics' && session && (
            <div className="v compact" key="lyrics" data-testid="view-lyrics">
              {music.artwork && <Artwork session={session} className="mini" />}
              <div className="grow" style={{ fontFamily: music.lyricsFont }}>
                {music.lyricsStyle === 'letters' && lyrics.doc && activeIndex >= 0 ? (
                  <div className="compact-letters" data-testid="compact-lyric">
                    <LetterLine
                      key={`c${activeIndex}`}
                      line={lyrics.doc.lines[activeIndex]}
                      endMs={lineEndMs(lyrics.doc.lines, activeIndex)}
                      getPos={getPos}
                      offsetMs={lyricsOffset}
                      playing={playing}
                      speed={music.lyricsAnimationSpeed}
                    />
                  </div>
                ) : (
                  <LineSwap text={activeText} speed={music.lyricsAnimationSpeed} />
                )}
              </div>
              {music.visualizer && <Visualizer playing={playing} />}
            </div>
          )}

          {layout.view === 'assistant' && (
            <div className="v compact assistant-compact" key="assistant" data-testid="view-assistant">
              <AssistantFace state={assistant.state} size={40} />
              <div className="grow compact-title">{ASSISTANT_LABEL[assistant.state]}</div>
            </div>
          )}

          {layout.view === 'expanded' && (
            <div className="v expanded" key="expanded" data-testid="view-expanded">
              <header className="bar-top">
                <nav className="tabs" role="tablist">
                  {TABS.map(({ id, label, icon: Icon }) => (
                    <button
                      key={id}
                      role="tab"
                      aria-selected={tab === id}
                      aria-label={label}
                      title={label}
                      className="tab"
                      data-active={tab === id}
                      data-testid={`tab-${id}`}
                      onClick={() => setTab(id)}
                    >
                      <Icon width={16} height={16} />
                    </button>
                  ))}
                </nav>
                <div className="tools">
                  <button
                    className="icon-btn small"
                    aria-label={exp.pinned ? 'Unpin' : 'Pin open'}
                    aria-pressed={exp.pinned}
                    title={exp.pinned ? 'Unpin' : 'Pin open'}
                    data-active={exp.pinned}
                    data-testid="btn-pin"
                    onClick={exp.togglePinned}
                  >
                    <PinIcon width={15} height={15} />
                  </button>
                  <button className="icon-btn small" aria-label="Settings" title="Settings" data-testid="btn-settings" onClick={() => void backend.openSettings()}>
                    <GearIcon width={15} height={15} />
                  </button>
                  <button className="icon-btn small" aria-label="Quit VEX" title="Quit VEX" data-testid="btn-quit" onClick={() => void backend.quit()}>
                    <PowerIcon width={15} height={15} />
                  </button>
                </div>
              </header>
              <main className="body" key={tab}>
                {tab === 'music' && <MusicPanel session={session} music={music} getPos={getPos} peek={<LyricPeek lyrics={lyrics} activeIndex={activeIndex} getPos={getPos} music={music} playing={playing} />} />}
                {tab === 'lyrics' && (
                  <div className="lyrics-tab">
                    {session && music.artwork && <Artwork session={session} className="side" />}
                    <LyricsView lyrics={lyrics} session={session} activeIndex={activeIndex} getPos={getPos} music={music} onSeek={seek} />
                  </div>
                )}
                {tab === 'assistant' && <AssistantPanel assistant={assistant} onFocusChange={setInputFocused} />}
              </main>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
