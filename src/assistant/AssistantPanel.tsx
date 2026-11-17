import { useEffect, useRef, useState, type FormEvent } from 'react';
import type { AssistantHook } from '../backend/hooks';
import type { AiProvider, ChatMessage, ContextPayload, Settings } from '../shared/types';
import { ClockIcon, MicIcon, PlusIcon, SendIcon, StopIcon, TrashIcon } from '../island/icons';
import { AssistantFace } from './AssistantFace';
import { detectContextRequest, type ContextWants } from './context';
import { Markdown } from './Markdown';
import type { ChatHook } from './useChat';

export const PROVIDER_NAMES: Record<AiProvider, string> = {
  ollama: 'Ollama',
  openai: 'OpenAI',
  gemini: 'Gemini',
  claude: 'Claude',
  openrouter: 'OpenRouter',
  'openai-compatible': 'Custom server',
};

const timeLabel = (ms: number) => {
  const d = new Date(ms);
  const today = new Date();
  const hm = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  return d.toDateString() === today.toDateString() ? hm : `${d.toLocaleDateString([], { month: 'short', day: 'numeric' })}, ${hm}`;
};

function Bubble({ m }: { m: ChatMessage }) {
  return (
    <div className="msg" data-role={m.role} data-testid={`msg-${m.role}`}>
      {m.role === 'user' ? <div className="bubble">{m.content}</div> : <Markdown text={m.content} />}
      <div className="msg-meta">
        {m.contextLabel && <span className="ctx-chip" title="This was sent to the model along with your message" data-testid="ctx-chip">Shared: {m.contextLabel}</span>}
        {m.partial && m.role === 'assistant' && <span data-testid="partial-note">stopped early</span>}
        <time title={new Date(m.tsMs).toLocaleString()}>{timeLabel(m.tsMs)}</time>
      </div>
    </div>
  );
}

/** What the reader sees while no answer text exists yet: dots plus honest elapsed time. */
function WaitNote({ phase, startedAt }: { phase: 'waiting' | 'thinking'; startedAt: number | null }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(id);
  }, []);
  const secs = startedAt ? Math.max(0, Math.floor((now - startedAt) / 1000)) : 0;
  const label = phase === 'thinking' ? 'Thinking' : secs >= 4 ? 'Waiting for the model (it may be loading)' : 'Starting';
  return (
    <div className="msg" data-role="assistant" data-testid="typing" data-phase={phase}>
      <div className="typing" aria-label="VEX is thinking"><i /><i /><i /></div>
      <div className="wait-note" data-testid="wait-note">{label}{secs >= 2 ? ` · ${secs}s` : ''}</div>
    </div>
  );
}

export function AssistantPanel({
  assistant,
  chat,
  ai,
  hasSession,
  buildContext,
  onOpenSettings,
  onFocusChange,
}: {
  assistant: AssistantHook;
  chat: ChatHook;
  ai: Settings['ai'];
  hasSession: boolean;
  /** Builds the context for ONE message from what it asks for. Called only at send time. */
  buildContext(wants: ContextWants): ContextPayload | null;
  onOpenSettings(): void;
  onFocusChange(focused: boolean): void;
}) {
  const [text, setText] = useState('');
  const [showHistory, setShowHistory] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [attach, setAttach] = useState<ContextWants>({ song: false, lyrics: false });
  const scroller = useRef<HTMLDivElement>(null);
  const stick = useRef(true);

  const busy = chat.phase !== 'idle';
  const model = ai.models[ai.provider] ?? '';
  const empty = chat.messages.length === 0 && chat.streaming === null && chat.phase === 'idle';

  // keep the newest text in view unless the reader scrolled up
  useEffect(() => {
    const el = scroller.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [chat.messages, chat.streaming, chat.phase, chat.error]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const t = text.trim();
    if (!t || busy) return;
    // Context goes out only when the person asked about the music (their words, or a chip they pressed).
    const auto = ai.contextOnRequest ? detectContextRequest(t) : { song: false, lyrics: false };
    const wants = { song: attach.song || auto.song, lyrics: attach.lyrics || auto.lyrics };
    const ok = await chat.send(t, buildContext(wants));
    if (ok) {
      setText('');
      setAttach({ song: false, lyrics: false });
      stick.current = true;
    }
  };

  const status = assistant.state === 'error' && assistant.message ? assistant.message : { idle: 'Ask me anything', listening: 'Listening…', thinking: 'Thinking…', speaking: 'Speaking…', error: 'Something went wrong' }[assistant.state];
  const needsSetup = chat.error?.kind === 'config' || chat.error?.kind === 'auth';

  return (
    <div className="assistant-panel chat" data-testid="assistant-panel">
      <div className="chat-head">
        <button type="button" className="chip-btn" onClick={onOpenSettings} title="Change provider or model in Settings" data-testid="model-chip">
          <span className="dot" data-ready={!!model} />
          {PROVIDER_NAMES[ai.provider]} · {model || 'choose a model'}
        </button>
        <span className="chat-title" title={chat.title} data-testid="chat-title">{chat.messages.length ? chat.title : ''}</span>
        <div className="chat-tools">
          <button type="button" className="icon-btn small" aria-label="Chat history" title="Chat history" aria-pressed={showHistory} data-active={showHistory} data-testid="btn-history" onClick={() => { setShowHistory((v) => !v); setConfirmClear(false); }}>
            <ClockIcon width={15} height={15} />
          </button>
          <button type="button" className="icon-btn small" aria-label="New chat" title="New chat" disabled={busy} data-testid="btn-new-chat" onClick={() => { chat.newChat(); setShowHistory(false); }}>
            <PlusIcon width={15} height={15} />
          </button>
        </div>
      </div>

      {showHistory ? (
        <div className="history" data-testid="history-panel">
          {chat.history.length === 0 && <div className="empty-note">No saved chats yet.</div>}
          <ul>
            {chat.history.map((h) => (
              <li key={h.id} data-active={h.id === chat.conversationId}>
                <button type="button" className="h-open" disabled={busy} onClick={() => { void chat.open(h.id); setShowHistory(false); }} data-testid="history-item">
                  <span className="h-title">{h.title}</span>
                  <span className="h-meta">{timeLabel(h.updatedMs)} · {h.messageCount} messages</span>
                </button>
                <button type="button" className="icon-btn small" aria-label={`Delete chat ${h.title}`} title="Delete" data-testid="history-delete" onClick={() => void chat.remove(h.id)}>
                  <TrashIcon width={14} height={14} />
                </button>
              </li>
            ))}
          </ul>
          {chat.history.length > 0 && (
            <div className="history-foot">
              {confirmClear ? (
                <>
                  <span>Delete all saved chats?</span>
                  <button type="button" className="pill-btn danger" data-testid="confirm-clear-history" onClick={() => { void chat.clearAll(); setConfirmClear(false); setShowHistory(false); }}>Delete all</button>
                  <button type="button" className="link-btn" onClick={() => setConfirmClear(false)}>Cancel</button>
                </>
              ) : (
                <button type="button" className="link-btn" data-testid="clear-history" onClick={() => setConfirmClear(true)}>Clear all history…</button>
              )}
            </div>
          )}
        </div>
      ) : (
        <div
          className="chat-scroll"
          ref={scroller}
          data-testid="chat-scroll"
          onScroll={(e) => {
            const el = e.currentTarget;
            stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
          }}
        >
          {empty && (
            <div className="chat-empty">
              <AssistantFace state={assistant.state} size={72} />
              <div className="assistant-status" data-testid="assistant-status" data-state={assistant.state}>{status}</div>
              {!model && <div className="setup-hint" data-testid="setup-hint">Pick a provider and model in <button type="button" className="link-btn" onClick={onOpenSettings}>Settings</button> to start chatting.</div>}
            </div>
          )}
          {chat.messages.map((m, i) => <Bubble key={`${m.tsMs}-${i}`} m={m} />)}
          {(chat.phase === 'waiting' || chat.phase === 'thinking') && <WaitNote phase={chat.phase} startedAt={chat.startedAt} />}
          {chat.streaming !== null && (
            <div className="msg streaming" data-role="assistant" data-testid="msg-streaming">
              <Markdown text={chat.streaming} />
            </div>
          )}
          {chat.error && (
            <div className="chat-error" role="alert" data-testid="chat-error">
              <span>{chat.error.message}</span>
              <span className="chat-error-actions">
                {needsSetup && <button type="button" className="link-btn" onClick={onOpenSettings}>Open Settings</button>}
                <button type="button" className="link-btn" onClick={chat.dismissError}>Dismiss</button>
              </span>
            </div>
          )}
        </div>
      )}

      <div className="attach-row">
        {hasSession && (
          <>
            <button type="button" className="chip-toggle" aria-pressed={attach.song} data-on={attach.song} data-testid="attach-song" onClick={() => setAttach((a) => ({ ...a, song: !a.song }))} title="Share the song that's playing with your next message">♪ Share song</button>
            <button type="button" className="chip-toggle" aria-pressed={attach.lyrics} data-on={attach.lyrics} data-testid="attach-lyrics" onClick={() => setAttach((a) => ({ ...a, lyrics: !a.lyrics }))} title="Share the current lyrics with your next message">Share lyrics</button>
          </>
        )}
        <span className="attach-note">{ai.contextOnRequest ? 'Music is shared only when you ask about it.' : 'Music is never shared unless you press a button.'}</span>
      </div>
      <form className="assistant-form" onSubmit={submit}>
        <button type="button" className="icon-btn" disabled title="Voice input arrives in a later milestone" aria-label="Voice input (not available yet)">
          <MicIcon />
        </button>
        <input
          data-testid="assistant-input"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onFocus={() => onFocusChange(true)}
          onBlur={() => onFocusChange(false)}
          placeholder={busy ? 'VEX is answering…' : 'Type a message…'}
          aria-label="Message to the assistant"
          maxLength={2000}
        />
        {busy ? (
          <button type="button" className="icon-btn accent" aria-label="Stop" title="Stop answering" data-testid="assistant-stop" onClick={chat.stop}>
            <StopIcon />
          </button>
        ) : (
          <button type="submit" className="icon-btn accent" disabled={!text.trim()} aria-label="Send" data-testid="assistant-send">
            <SendIcon />
          </button>
        )}
      </form>
    </div>
  );
}
