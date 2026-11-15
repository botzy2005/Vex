/**
 * Equalizer-style "now playing" indicator. It reflects play/pause state only;
 * it is NOT audio-reactive (real audio analysis is a later milestone).
 */
export function Visualizer({ playing }: { playing: boolean }) {
  return (
    <span className="viz" data-playing={playing} data-testid="visualizer" aria-hidden>
      <i /><i /><i /><i />
    </span>
  );
}
