import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { AssistantFace } from '../assistant/AssistantFace';
import { AssistantPanel } from '../assistant/AssistantPanel';
import { buildContext } from '../assistant/context';
import { useChat } from '../assistant/useChat';
import { useAssistant, useBackend, useMedia, useResolvedTheme, useSettings, useSubscription } from '../backend/hooks';
import { LetterLine } from '../lyrics/LetterLine';
import { LineSwap } from '../lyrics/LineSwap';
import { lineEndMs } from '../lyrics/sync';
import { LyricPeek } from '../lyrics/LyricPeek';
import { LyricsView } from '../lyrics/LyricsView';
import { useLyrics } from '../lyrics/useLyrics';
import { NotificationPreview, NotificationsPanel } from '../notifications/NotificationsPanel';
import { useNotifications } from '../notifications/useNotifications';
import { ClipboardPanel } from '../clipboard/ClipboardPanel';
import { useClipboard } from '../clipboard/useClipboard';
import { GridIcon } from '../hub/icons';
import { WidgetsPanel } from '../hub/WidgetsPanel';
import { ScreenshotErrorToast, ScreenshotPanel, ScreenshotToast } from '../screenshot/ScreenshotPanel';
import { useScreenshotToast } from '../screenshot/useScreenshots';
import { BellIcon, CameraIcon, ClipboardIcon, GearIcon, LyricsIcon, MusicIcon, PinIcon, PowerIcon, SparkIcon } from './icons';
import { onAccent } from './accent';
import { useIslandDrag } from './useIslandDrag';
import { useAutoHide } from './useAutoHide';
import { useAccent, useActiveLine, useExpansion, useIslandSpring, usePlaybackPosition, useWindowSize } from './hooks';
import { computeLayout, resolveCompactView, type Tab } from './layout';
import { MediaBackdrop } from './MediaBackdrop';
import { Artwork, MusicPanel } from './MusicPanel';
import { Transport } from './Transport';
import { Visualizer } from './Visualizer';

const TABS: Array<{ id: Tab; label: string; icon: typeof MusicIcon }> = [
  { id: 'music', label: 'Music', icon: MusicIcon },
  { id: 'lyrics', label: 'Lyrics', icon: LyricsIcon },
  { id: 'assistant', label: 'Assistant', icon: SparkIcon },
  { id: 'notifications', label: 'Notifications', icon: BellIcon },
  { id: 'clipboard', label: 'Clipboard', icon: ClipboardIcon },
  { id: 'screenshot', label: 'Screenshot', icon: CameraIcon },
  { id: 'widgets', label: 'Widgets', icon: GridIcon },
];

const ASSISTANT_LABEL = { idle: '', listening: 'Listening', thinking: 'Thinking', speaking: 'Speaking', error: 'Error' } as const;

export function IslandApp() {
  const backend = useBackend();
  const { settings } = useSettings();
  const media = useMedia();
  const assistant = useAssistant();
  const chat = useChat(backend, assistant);
  const session = media.session;
  const notif = useNotifications(true);
  const clip = useClipboard(false);
  const shot = useScreenshotToast(settings?.screenshot);
  const [previewId, setPreviewId] = useState<number | null>(null);

  const [tab, setTab] = useState<Tab>('music');
  const [inputFocused, setInputFocused] = useState(false);
  const drag = useIslandDrag(backend, settings?.island.offsetX ?? 0);
  const exp = useExpansion(settings?.island, inputFocused || drag.dragging, !!shot.toast);

  const hide = useAutoHide(backend, settings?.island.autoHideDelayMs ?? 800, {
    mode: settings?.island.visibility ?? 'always',
    expanded: exp.expanded,
    pinned: exp.pinned,
    inputFocused,
    toast: !!shot.toast,
    dragging: drag.dragging,
    assistantBusy: assistant.state === 'listening' || assistant.state === 'thinking' || assistant.state === 'speaking',
  });
  // Only these reveal a hidden island: the edge strip, the shortcut / tray "Show VEX" (island-reveal) and a
  // screenshot confirmation. Notifications, timers and widget updates deliberately do not.
  useSubscription<null>((cb) => backend.onIslandReveal(() => cb(null)), () => hide.reveal());
  const shotShowing = !!shot.toast;
  useEffect(() => {
    if (shotShowing) hide.reveal();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shotShowing]);

  const getPos = usePlaybackPosition(session);
  const music = settings?.music;
  const lyrics = useLyrics(session, backend, music?.lyricsOnline ?? true);
  const lyricsOffset = music?.lyricsOffsetMs ?? 0;
  const activeIndex = useActiveLine(music?.lyricsEnabled ? lyrics.doc : null, getPos, lyricsOffset, session);
  const activeText = lyrics.doc && activeIndex >= 0 ? lyrics.doc.lines[activeIndex].text : '';
  const accent = useAccent(settings, session?.artworkUrl);
  const theme = useResolvedTheme(settings?.appearance.theme);

  useSubscription<null>((cb) => backend.onIslandToggle(() => cb(null)), () => exp.toggleFromHotkey());

  // Expanding while a notification preview is showing (by hover or hotkey) lands on the notification panel.
  const previewShowing = !!notif.preview;
  useEffect(() => {
    if (exp.expanded && previewShowing) setTab('notifications');
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [exp.expanded]);

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
    notification: !!notif.preview && !!settings?.notifications.enabled,
    screenshot: !!shot.toast,
  });
  const layout = computeLayout({
    expanded: exp.expanded,
    tab,
    compact,
    sizeScale: a?.sizeScale ?? 1,
    shape: a?.shape ?? 'pill',
    cornerRadius: a?.cornerRadius ?? 26,
  });

  const mediaBackdrop = exp.expanded && (tab === 'music' || tab === 'lyrics') && !!session?.artworkUrl && !!music?.artwork;
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
      data-hidden={hide.phase !== 'visible'}
      data-hide-phase={hide.phase}
      style={{ '--accent': accent, '--on-accent': onAccent(accent), '--glass': a.glassOpacity, '--speed': music.lyricsAnimationSpeed } as CSSProperties}
    >
      {hide.phase === 'hidden' && (
        <div className="edge-strip" data-testid="edge-strip" aria-hidden="true" onPointerEnter={() => hide.reveal(true)} />
      )}
      <div
        ref={islandRef}
        className="island"
        data-testid="island"
        data-view={layout.view}
        data-expanded={exp.expanded}
        data-pinned={exp.pinned}
        data-shape={a.shape}
        data-assistant={assistant.state}
        data-media={mediaBackdrop}
        data-dragging={drag.dragging}
        onPointerDown={drag.onIslandPointerDown}
        onPointerEnter={() => {
          hide.onPointerEnter();
          exp.onPointerEnter();
        }}
        onPointerLeave={() => {
          hide.onPointerLeave();
          exp.onPointerLeave();
        }}
        onClick={() => {
          if (exp.expanded) return;
          if (layout.view === 'notification') setTab('notifications'); // a click on the preview opens the full panel
          if (layout.view === 'screenshot') {
            if (shot.toast?.kind === 'shot') setPreviewId(shot.toast.shot.id);
            setTab('screenshot');
            shot.dismiss();
          }
          exp.expand();
        }}
      >
        {session?.artworkUrl && music.artwork && <MediaBackdrop url={session.artworkUrl} on={mediaBackdrop} />}
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

          {layout.view === 'notification' && notif.preview && (
            <NotificationPreview
              key={`n${notif.preview.id}`}
              entry={notif.preview}
              icon={notif.view.apps.find((a) => a.id === notif.preview!.appId)?.icon ?? null}
              onDismiss={() => {
                void backend.notificationDismiss(notif.preview!.id);
                notif.hidePreview();
              }}
            />
          )}

          {layout.view === 'screenshot' && shot.toast?.kind === 'shot' && (
            <ScreenshotToast key={`s${shot.toast.shot.id}`} shot={shot.toast.shot} onKeep={shot.extend} onDismiss={shot.dismiss} />
          )}
          {layout.view === 'screenshot' && shot.toast?.kind === 'error' && <ScreenshotErrorToast message={shot.toast.message} onDismiss={shot.dismiss} />}

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
                      {id === 'clipboard' && clip.status.recording && <span className="tab-rec" data-testid="clip-rec-dot" title="Clipboard history is recording" />}
                      {id === 'notifications' && settings.notifications.enabled && notif.view.unread > 0 && (
                        <span className="tab-badge" data-testid="notif-badge">{notif.view.unread > 9 ? '9+' : notif.view.unread}</span>
                      )}
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
                  <div className="lyrics-tab" data-testid="lyrics-tab">
                    {session && (
                      <header className="lyrics-head">
                        {music.artwork && <Artwork session={session} className="side" />}
                        <div className="lyrics-titles">
                          <div className="title small" title={session.title}>{session.title}</div>
                          <div className="artist small" title={session.artist}>{session.artist}</div>
                        </div>
                        <Transport session={session} onError={() => {}} compact />
                      </header>
                    )}
                    <LyricsView lyrics={lyrics} session={session} activeIndex={activeIndex} getPos={getPos} music={music} onSeek={seek} />
                  </div>
                )}
                {tab === 'clipboard' && <ClipboardPanel enabled={settings.clipboard.enabled} onOpenSettings={() => void backend.openSettings()} />}
                {tab === 'screenshot' && (
                  <ScreenshotPanel hotkey={settings.screenshot.hotkey} hotkeyMode={settings.screenshot.hotkeyMode} previewId={previewId} onPreview={setPreviewId} onBeforeCapture={exp.collapse} onCaptureError={shot.fail} />
                )}
                {tab === 'widgets' && <WidgetsPanel />}
                {tab === 'notifications' && <NotificationsPanel view={notif.view} settings={settings.notifications} onOpenSettings={() => void backend.openSettings()} />}
                {tab === 'assistant' && (
                  <AssistantPanel
                    assistant={assistant}
                    chat={chat}
                    ai={settings.ai}
                    hasSession={!!session}
                    onOpenSettings={() => void backend.openSettings()}
                    onFocusChange={setInputFocused}
                    buildContext={(wants) => buildContext(wants, { session, lyrics: lyrics.doc, plainLyrics: lyrics.plain, activeIndex, positionMs: getPos() })}
                  />
                )}
              </main>
            </div>
          )}
          <button
            type="button"
            className="drag-grip"
            data-testid="drag-grip"
            aria-label="Move the island along the top edge (or hold Alt and drag it)"
            title="Drag to move the island (or hold Alt and drag)"
            tabIndex={-1}
            onPointerDown={drag.onGripPointerDown(islandRef.current)}
          />
        </div>
      </div>
    </div>
  );
}
