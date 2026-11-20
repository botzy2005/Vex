import { useState, type ReactNode } from 'react';
import type { MediaSession, Settings } from '../shared/types';
import { MusicIcon } from './icons';
import { ProgressBar } from './ProgressBar';
import { Transport } from './Transport';
import { Visualizer } from './Visualizer';

export function Artwork({ session, className }: { session: MediaSession | null; className?: string }) {
  return (
    <div className={`artwork ${className ?? ''}`} data-testid="artwork">
      {session?.artworkUrl ? <img src={session.artworkUrl} alt="" draggable={false} /> : <MusicIcon width="45%" height="45%" />}
    </div>
  );
}

export function MusicPanel({
  session,
  music,
  getPos,
  peek,
}: {
  session: MediaSession | null;
  music: Settings['music'];
  getPos: () => number;
  /** Current lyric line, shown between the titles and the progress bar. */
  peek?: ReactNode;
}) {
  const [error, setError] = useState<string | null>(null);

  if (!session) {
    return (
      <div className="empty" data-testid="music-empty">
        <MusicIcon width={28} height={28} />
        <strong>Nothing playing</strong>
        <span>Play something in Spotify, YouTube Music, a browser tab or any media player.</span>
      </div>
    );
  }

  return (
    <div className="music-panel" data-testid="music-panel" data-art={music.artwork}>
      {music.artwork && <Artwork session={session} className="large" />}
      <div className="music-head">
        <div className="titles">
          <div className="title-row">
            <div className="title" data-testid="track-title" title={session.title}>{session.title}</div>
            {music.visualizer && <Visualizer playing={session.status === 'playing'} />}
          </div>
          <div className="artist" data-testid="track-artist" title={session.artist}>{session.artist}</div>
        </div>
        <div className="music-stage">{peek}</div>
      </div>
      <div className="music-foot">
        <ProgressBar session={session} getPos={getPos} onError={setError} />
        <Transport session={session} onError={setError} />
        {error && <div className="inline-error" role="alert" onAnimationEnd={() => setError(null)}>{error}</div>}
      </div>
    </div>
  );
}
