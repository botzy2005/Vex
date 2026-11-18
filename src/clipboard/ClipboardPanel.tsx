import { useEffect, useRef, useState } from 'react';
import { useBackend } from '../backend/hooks';
import { CloseIcon, PinIcon } from '../island/icons';
import type { ClipboardItem, ClipboardStatus } from '../shared/types';
import { describeItem, sensitiveNote, statusLine } from './format';
import { useClipboard } from './useClipboard';

export function ClipboardPanel({ enabled, onOpenSettings }: { enabled: boolean; onOpenSettings(): void }) {
  const backend = useBackend();
  const [query, setQuery] = useState('');
  const { status, items } = useClipboard(true, query);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const line = statusLine(status);
  const privacy = sensitiveNote(status.skippedSensitive);

  const copy = async (i: ClipboardItem) => {
    setError(null);
    try {
      await backend.clipboardCopy(i.id);
      setCopiedId(i.id);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopiedId(null), 1400);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    }
  };
  const act = async (f: () => Promise<void>) => {
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    }
  };

  return (
    <div className="clip-panel" data-testid="clipboard-panel">
      <div className="clip-head">
        <span className="clip-state" data-testid="clip-status" data-tone={line.tone} data-recording={status.recording}>
          <i className="clip-dot" aria-hidden />
          {line.text}
        </span>
        <span className="grow" />
        <button className="pill-btn" data-testid="clip-clear" disabled={status.count === status.pinned} onClick={() => void act(() => backend.clipboardClear(true))} title="Remove everything except pinned entries">
          Clear
        </button>
      </div>
      {!enabled ? (
        <div className="notif-empty" data-testid="clip-off">
          <p>Clipboard history is off. Turn it on in Settings to keep what you copy on this PC.</p>
          <button className="pill-btn" onClick={onOpenSettings}>Open settings</button>
        </div>
      ) : (
        <>
          <input
            className="clip-search"
            type="search"
            placeholder="Search copied text"
            aria-label="Search clipboard history"
            data-testid="clip-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          {(error || status.note || privacy) && (
            <div className="notif-note" role="status" data-testid="clip-note">{error ?? status.note ?? privacy}</div>
          )}
          {items.length === 0 ? (
            <div className="notif-empty" data-testid="clip-empty">
              <p>{query ? 'No copied text matches.' : status.recording ? 'Nothing yet. Copy some text or an image.' : 'Nothing saved.'}</p>
            </div>
          ) : (
            <div className="clip-list" data-testid="clip-list">
              {items.map((i) => (
                <ClipRow key={i.id} item={i} copied={copiedId === i.id} onCopy={() => void copy(i)} onPin={() => void act(() => backend.clipboardPin(i.id, !i.pinned))} onDelete={() => void act(() => backend.clipboardDelete(i.id))} />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}

function ClipRow({ item, copied, onCopy, onPin, onDelete }: { item: ClipboardItem; copied: boolean; onCopy(): void; onPin(): void; onDelete(): void }) {
  return (
    <div className="clip-item" data-testid="clip-item" data-kind={item.kind} data-pinned={item.pinned}>
      <button className="clip-main" onClick={onCopy} title="Copy back to the clipboard" data-testid="clip-copy">
        {item.kind === 'image' && item.thumb ? <img className="clip-thumb" src={item.thumb} alt="Copied image" /> : <span className="clip-preview">{item.preview || '(empty)'}</span>}
        <span className="clip-meta">{copied ? 'Copied' : describeItem(item)}</span>
      </button>
      <button className="icon-btn small" aria-label={item.pinned ? 'Unpin entry' : 'Pin entry'} aria-pressed={item.pinned} data-active={item.pinned} title={item.pinned ? 'Unpin' : 'Pin'} data-testid="clip-pin" onClick={onPin}>
        <PinIcon width={13} height={13} />
      </button>
      <button className="icon-btn small" aria-label="Delete entry" title="Delete" data-testid="clip-delete" onClick={onDelete}>
        <CloseIcon width={12} height={12} />
      </button>
    </div>
  );
}

export type { ClipboardStatus };
