import { useState, type FormEvent } from 'react';
import type { AssistantHook } from '../backend/hooks';
import type { AssistantState } from '../shared/types';
import { MicIcon, SendIcon } from '../island/icons';
import { AssistantFace } from './AssistantFace';

const STATUS: Record<AssistantState, string> = {
  idle: 'Ask me anything',
  listening: 'Listening…',
  thinking: 'Thinking…',
  speaking: 'Speaking…',
  error: 'Something went wrong',
};

export function AssistantPanel({
  assistant,
  onFocusChange,
}: {
  assistant: AssistantHook;
  onFocusChange(focused: boolean): void;
}) {
  const [text, setText] = useState('');
  const busy = assistant.state === 'thinking';

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!text.trim() || busy) return;
    const t = text;
    setText('');
    void assistant.send(t);
  };

  return (
    <div className="assistant-panel" data-testid="assistant-panel">
      <div className="assistant-face-wrap">
        <AssistantFace state={assistant.state} size={104} />
      </div>
      <div className="assistant-status" data-testid="assistant-status" data-state={assistant.state}>
        {assistant.state === 'error' && assistant.message ? assistant.message : STATUS[assistant.state]}
      </div>
      <form className="assistant-form" onSubmit={submit}>
        <button
          type="button"
          className="icon-btn"
          disabled
          title="Voice input arrives in Milestone 6"
          aria-label="Voice input (not available yet)"
        >
          <MicIcon />
        </button>
        <input
          data-testid="assistant-input"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onFocus={() => onFocusChange(true)}
          onBlur={() => onFocusChange(false)}
          placeholder="Type a message…"
          aria-label="Message to the assistant"
          maxLength={2000}
        />
        <button type="submit" className="icon-btn accent" disabled={!text.trim() || busy} aria-label="Send" data-testid="assistant-send">
          <SendIcon />
        </button>
      </form>
    </div>
  );
}
