import { useEffect, useRef, useState } from 'react';
import { useBackend } from '../backend/hooks';
import { CloseIcon } from '../island/icons';
import { relativeTime } from '../notifications/group';
import type { CaptureMode, Settings, ShotView } from '../shared/types';
import { useShotList } from './useScreenshots';

const MODES: Array<{ id: CaptureMode; label: string; hint: string }> = [
  { id: 'region', label: 'Select area', hint: 'Drag a rectangle on the screen' },
  { id: 'window', label: 'Window', hint: 'The window you are working in' },
  { id: 'screen', label: 'Full screen', hint: 'The display the pointer is on' },
];

const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;
const errText = (e: unknown) => String(e instanceof Error ? e.message : e);

/** Quick actions on one capture, shared by the island toast and the history rows. */
export function useShotActions(onDone?: () => void) {
  const backend = useBackend();
  const [note, setNote] = useState<{ text: string; kind: 'ok' | 'error'; path?: string } | null>(null);
  const run = async (f: () => Promise<{ text: string; path?: string } | null>) => {
    try {
      const r = await f();
      if (r) setNote({ text: r.text, kind: 'ok', path: r.path });
      onDone?.();
    } catch (e) {
      setNote({ text: errText(e), kind: 'error' });
    }
  };
  return {
    note,
    clearNote: () => setNote(null),
    edit: (s: ShotView) => run(async () => (await backend.screenshotOpenEditor(s.id), null)),
    copy: (s: ShotView) => run(async () => (await backend.screenshotCopy({ id: s.id }), { text: 'Copied to the clipboard' })),
    save: (s: ShotView, ask = false) =>
      run(async () => {
        const p = await backend.screenshotSave({ id: s.id }, ask);
        return p ? { text: `Saved ${baseName(p)}`, path: p } : null;
      }),
    reveal: (path: string) => run(async () => (await backend.screenshotReveal(path), null)),
  };
}

export function ScreenshotPanel({
  hotkey, hotkeyMode, previewId, onPreview, onBeforeCapture, onCaptureError,
}: {
  hotkey: Settings['screenshot']['hotkey'];
  hotkeyMode: CaptureMode;
  previewId: number | null;
  onPreview(id: number | null): void;
  /** Called just before a capture starts (the island collapses so it is out of the way). */
  onBeforeCapture(): void;
  /** A capture could not start. The panel is already gone by then, so the island reports it. */
  onCaptureError(message: string): void;
}) {
  const backend = useBackend();
  const { items, reload } = useShotList(true);
  const actions = useShotActions();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(t);
  }, []);

  const capture = async (mode: CaptureMode) => {
    onBeforeCapture();
    try {
      await backend.screenshotCapture(mode);
    } catch (e) {
      onCaptureError(errText(e));
    }
  };

  const previewShot = items.find((i) => i.id === previewId) ?? null;

  return (
    <div className="shot-panel" data-testid="screenshot-panel">
      <div className="shot-modes">
        {MODES.map((m) => (
          <button key={m.id} className="pill-btn" title={m.hint} data-testid={`shot-${m.id}`} onClick={() => void capture(m.id)}>
            {m.label}
          </button>
        ))}
        <span className="grow" />
        <button className="pill-btn" data-testid="shot-clear" disabled={items.length === 0} onClick={() => void backend.screenshotClear().then(reload)}>Clear</button>
      </div>
      <div className="shot-shortcut" data-testid="shot-shortcut">
        {hotkey ? <>Shortcut <kbd>{hotkey}</kbd> captures {hotkeyMode === 'region' ? 'an area' : hotkeyMode === 'window' ? 'the active window' : 'the full screen'}</> : 'No shortcut set. You can choose one in Settings.'}
      </div>
      {actions.note && (
        <div className="notif-note" role="status" data-testid="shot-note" data-kind={actions.note.kind}>
          {actions.note.text}
          {actions.note.path && <button className="shot-link" data-testid="shot-reveal" onClick={() => void actions.reveal(actions.note!.path!)}>Show in folder</button>}
        </div>
      )}
      {items.length === 0 ? (
        <div className="notif-empty" data-testid="shot-empty"><p>No screenshots yet. Use a button above or the shortcut.</p></div>
      ) : (
        <div className="shot-list" data-testid="shot-list">
          {items.map((s) => (
            <ShotRow key={s.id} shot={s} now={now} actions={actions} onPreview={() => onPreview(s.id)} onDelete={() => void backend.screenshotDelete(s.id).then(reload)} />
          ))}
        </div>
      )}
      {previewId !== null && <Preview id={previewId} shot={previewShot} actions={actions} onClose={() => onPreview(null)} />}
    </div>
  );
}

function ShotRow({ shot, now, actions, onPreview, onDelete }: { shot: ShotView; now: number; actions: ReturnType<typeof useShotActions>; onPreview(): void; onDelete(): void }) {
  return (
    <div className="shot-item" data-testid="shot-item" data-id={shot.id}>
      <button className="shot-thumb" onClick={onPreview} title="Preview" data-testid="shot-thumb" aria-label={`Preview screenshot ${shot.width} by ${shot.height}`}>
        <img src={shot.thumb} alt="" />
      </button>
      <div className="shot-meta">
        <span className="shot-dims">{shot.width} × {shot.height}</span>
        <span className="shot-time">{relativeTime(shot.tsMs, now)}</span>
      </div>
      <span className="grow" />
      <button className="pill-btn small" data-testid="shot-edit" onClick={() => void actions.edit(shot)}>Edit</button>
      <button className="pill-btn small" data-testid="shot-copy" onClick={() => void actions.copy(shot)}>Copy</button>
      <button className="pill-btn small" data-testid="shot-save" onClick={() => void actions.save(shot)}>Save</button>
      <button className="icon-btn small" aria-label="Delete screenshot" title="Delete" data-testid="shot-delete" onClick={onDelete}>
        <CloseIcon width={12} height={12} />
      </button>
    </div>
  );
}

function Preview({ id, shot, actions, onClose }: { id: number; shot: ShotView | null; actions: ReturnType<typeof useShotActions>; onClose(): void }) {
  const backend = useBackend();
  const [url, setUrl] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    let alive = true;
    setUrl(null);
    setErr(null);
    backend.screenshotImage(id).then((u) => alive && setUrl(u)).catch((e) => alive && setErr(errText(e)));
    return () => {
      alive = false;
      setUrl(null); // the full-size picture is not kept once the preview closes
    };
  }, [backend, id]);
  useEffect(() => closeRef.current?.focus(), []);
  return (
    <div className="shot-preview" role="dialog" aria-label="Screenshot preview" data-testid="shot-preview" onKeyDown={(e) => e.key === 'Escape' && (e.stopPropagation(), onClose())}>
      <div className="shot-preview-body">
        {url ? <img src={url} alt="Screenshot preview" data-testid="shot-preview-img" /> : <span className="ed-dim">{err ?? 'Loading…'}</span>}
      </div>
      <div className="shot-preview-bar">
        <span className="shot-dims">{shot ? `${shot.width} × ${shot.height}` : ''}</span>
        <span className="grow" />
        {shot && (
          <>
            <button className="pill-btn small" data-testid="preview-edit" onClick={() => void actions.edit(shot)}>Edit</button>
            <button className="pill-btn small" data-testid="preview-copy" onClick={() => void actions.copy(shot)}>Copy</button>
            <button className="pill-btn small" data-testid="preview-save" onClick={() => void actions.save(shot, true)}>Save as…</button>
          </>
        )}
        <button ref={closeRef} className="pill-btn small" data-testid="preview-close" onClick={onClose}>Close</button>
      </div>
    </div>
  );
}

/** The collapsed island's "screenshot captured" confirmation with quick actions. */
export function ScreenshotToast({
  shot, onKeep, onDismiss,
}: { shot: ShotView; onKeep(): void; onDismiss(): void }) {
  const actions = useShotActions(onKeep);
  const stop = (e: React.MouseEvent) => e.stopPropagation(); // buttons must not also expand the island
  return (
    <div className="v compact shot-toast" data-testid="view-screenshot">
      <img className="shot-toast-thumb" src={shot.thumb} alt="" data-testid="toast-thumb" />
      <div className="grow shot-toast-text">
        <span className="shot-toast-title" data-testid="toast-title">
          {actions.note ? actions.note.text : 'Screenshot captured'}
        </span>
        <span className="shot-toast-sub">{shot.width} × {shot.height}</span>
      </div>
      {actions.note?.path ? (
        <button className="pill-btn small" data-testid="toast-reveal" onClick={(e) => (stop(e), void actions.reveal(actions.note!.path!))}>Show</button>
      ) : (
        <>
          <button className="pill-btn small" data-testid="toast-edit" onClick={(e) => (stop(e), void actions.edit(shot).then(onDismiss))}>Edit</button>
          <button className="pill-btn small" data-testid="toast-copy" onClick={(e) => (stop(e), void actions.copy(shot))}>Copy</button>
          <button className="pill-btn small" data-testid="toast-save" onClick={(e) => (stop(e), void actions.save(shot))}>Save</button>
        </>
      )}
      <button className="icon-btn small" aria-label="Dismiss" data-testid="toast-dismiss" onClick={(e) => (stop(e), onDismiss())}>
        <CloseIcon width={12} height={12} />
      </button>
    </div>
  );
}

/** The collapsed island's message when a capture failed. */
export function ScreenshotErrorToast({ message, onDismiss }: { message: string; onDismiss(): void }) {
  return (
    <div className="v compact shot-toast" data-testid="view-screenshot-error" role="alert">
      <div className="grow shot-toast-text">
        <span className="shot-toast-title">Screenshot failed</span>
        <span className="shot-toast-sub" title={message}>{message}</span>
      </div>
      <button className="icon-btn small" aria-label="Dismiss" data-testid="toast-dismiss" onClick={(e) => (e.stopPropagation(), onDismiss())}>
        <CloseIcon width={12} height={12} />
      </button>
    </div>
  );
}
