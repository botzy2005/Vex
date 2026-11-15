import type { AssistantState } from '../shared/types';

/**
 * The assistant's face. Everything is driven by `data-state` in CSS
 * (see styles/island.css): blinking when idle, wide-eyed with a pulsing ring
 * when listening, scanning eyes and thinking dots, a talking mouth, and a
 * drooping error face.
 */
export function AssistantFace({ state, size = 40 }: { state: AssistantState; size?: number }) {
  return (
    <svg
      className="face"
      data-state={state}
      data-testid="assistant-face"
      viewBox="0 0 100 64"
      width={size}
      height={(size * 64) / 100}
      role="img"
      aria-label={`Assistant ${state}`}
    >
      <circle className="face-ring" cx="50" cy="32" r="30" />
      <g className="face-eyes">
        <rect className="face-eye face-eye-l" x="26" y="14" width="12" height="20" rx="6" />
        <rect className="face-eye face-eye-r" x="62" y="14" width="12" height="20" rx="6" />
      </g>
      <path className="face-mouth" />
      <g className="face-dots">
        <circle cx="42" cy="6" r="2.4" />
        <circle cx="50" cy="6" r="2.4" />
        <circle cx="58" cy="6" r="2.4" />
      </g>
    </svg>
  );
}
