import type { CSSProperties } from 'react';
import type { Settings, VolumeState } from '../shared/types';
import { VolumeIcon } from './icons';
import { clampPercent, volumeLabel, volumeLevel } from './volume';

interface Props {
  state: VolumeState | null;
  failure: string | null;
  step: Settings['island']['volumeStep'];
  onSlide(percent: number): void;
  onSlideEnd(): void;
  onStep(d: 1 | -1): void;
  onToggleMute(): void;
  onClose(): void;
}

/** Compact volume panel: slider, percentage, minus / plus and mute. Disabled with a reason when Windows has no output. */
export function VolumePopover({ state, failure, step, onSlide, onSlideEnd, onStep, onToggleMute, onClose }: Props) {
  const ready = !!state?.available;
  const percent = state ? clampPercent(state.percent) : 0;
  const muted = !!state?.muted;
  const message = failure ?? state?.error ?? (state ? null : 'Reading the volume…');
  return (
    <div className="volume-pop" data-testid="volume-popover" role="dialog" aria-label="Volume" onKeyDown={(e) => e.key === 'Escape' && (e.stopPropagation(), onClose())}>
      <div className="volume-row">
        <button type="button" className="icon-btn small" data-testid="volume-mute" aria-label={muted ? 'Unmute' : 'Mute'} aria-pressed={muted} title={muted ? 'Unmute' : 'Mute'} disabled={!ready} onClick={onToggleMute}>
          <VolumeIcon level={volumeLevel(percent, muted)} width={16} height={16} />
        </button>
        <button type="button" className="icon-btn small volume-step" data-testid="volume-down" aria-label={`Volume down ${step}%`} title={`Down ${step}%`} disabled={!ready || percent === 0} onClick={() => onStep(-1)}>−</button>
        <input
          type="range"
          className="volume-slider"
          data-testid="volume-slider"
          aria-label="Volume"
          min={0}
          max={100}
          step={1}
          value={percent}
          disabled={!ready}
          style={{ '--fill': `${muted ? 0 : percent}%` } as CSSProperties}
          onChange={(e) => onSlide(Number(e.target.value))}
          onPointerUp={onSlideEnd}
          onKeyUp={onSlideEnd}
          onBlur={onSlideEnd}
        />
        <button type="button" className="icon-btn small volume-step" data-testid="volume-up" aria-label={`Volume up ${step}%`} title={`Up ${step}%`} disabled={!ready || percent === 100} onClick={() => onStep(1)}>+</button>
        <span className="volume-pct" data-testid="volume-percent" aria-live="polite">{ready ? volumeLabel(percent, muted) : '–'}</span>
      </div>
      {message && <div className="volume-msg" data-testid="volume-message" role="status">{message}</div>}
    </div>
  );
}
