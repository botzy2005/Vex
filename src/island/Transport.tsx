import { useBackend } from '../backend/hooks';
import type { MediaSession } from '../shared/types';
import { NextIcon, PauseIcon, PlayIcon, PrevIcon } from './icons';

export function Transport({ session, onError, compact = false }: { session: MediaSession; onError(msg: string): void; compact?: boolean }) {
  const backend = useBackend();
  const c = session.capabilities;
  const playing = session.status === 'playing';
  const run = (cmd: Parameters<typeof backend.mediaTransport>[0]) =>
    backend.mediaTransport(cmd).catch((e) => onError(e instanceof Error ? e.message : String(e)));

  return (
    <div className="transport" data-compact={compact} role="group" aria-label="Playback controls">
      <button className="icon-btn" disabled={!c.previous} onClick={() => run({ type: 'previous' })} aria-label="Previous track" data-testid="btn-prev">
        <PrevIcon width={20} height={20} />
      </button>
      <button
        className="icon-btn play"
        disabled={playing ? !c.pause : !c.play}
        onClick={() => run({ type: 'toggle' })}
        aria-label={playing ? 'Pause' : 'Play'}
        data-testid="btn-toggle"
      >
        {playing ? <PauseIcon width={24} height={24} /> : <PlayIcon width={24} height={24} />}
      </button>
      <button className="icon-btn" disabled={!c.next} onClick={() => run({ type: 'next' })} aria-label="Next track" data-testid="btn-next">
        <NextIcon width={20} height={20} />
      </button>
    </div>
  );
}
