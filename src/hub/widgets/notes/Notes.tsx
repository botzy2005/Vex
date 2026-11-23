import { useEffect, useMemo, useRef, useState } from 'react';
import type { WidgetProps } from '../../registry';
import { useWidgetData } from '../../useWidgetData';
import {
  addNote, ago, canAdd, EMPTY_DOC, MAX_BODY, normalize, previewOf, removeNote, search, setBody, titleOf, togglePin, type NotesDoc,
} from './logic';
import './notes.css';

/** Notes: a searchable list and a plain-text editor. Everything is saved on this PC a moment after each edit. */
export default function Notes({ size }: WidgetProps) {
  const { value: doc, update, loaded, error, flush } = useWidgetData<NotesDoc>('notes', EMPTY_DOC, normalize);
  const [openId, setOpenId] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const area = useRef<HTMLTextAreaElement>(null);

  const open = openId ? doc.notes.find((n) => n.id === openId) : undefined;
  const list = useMemo(() => search(doc.notes, query), [doc.notes, query]);

  // Relative times ("5 min ago") refresh only while the list is showing, once a minute.
  useEffect(() => {
    if (open) return;
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, [open]);

  useEffect(() => {
    const el = area.current;
    if (!open || !el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length); // continue where the text ends
  }, [openId]); // eslint-disable-line react-hooks/exhaustive-deps

  const create = () => {
    const ok = canAdd(doc);
    if (!ok.ok) return setMessage(ok.reason);
    setMessage(null);
    const r = addNote(doc, Date.now());
    update(r.doc);
    setQuery('');
    setOpenId(r.id);
  };

  /** Leave the editor. A note that was never typed into is dropped instead of leaving an empty row behind. */
  const back = () => {
    if (open && open.body.trim() === '') update((d) => removeNote(d, open.id));
    setOpenId(null);
    setConfirmDelete(false);
    flush();
  };

  if (!loaded) return <div className="notes" data-testid="widget-notes" data-view="loading" />;

  if (open) {
    const left = MAX_BODY - open.body.length;
    return (
      <div className="notes" data-size={size} data-view="editor" data-testid="widget-notes">
        <div className="notes-bar">
          <button className="hub-btn" data-testid="note-back" onClick={back}>← Notes</button>
          <span className="spacer" />
          <button className="hub-btn" data-testid="note-pin" aria-pressed={open.pinned} onClick={() => update((d) => togglePin(d, open.id))}>{open.pinned ? 'Unpin' : 'Pin'}</button>
          {confirmDelete ? (
            <>
              <button className="hub-btn danger" data-testid="note-delete-yes" onClick={() => { update((d) => removeNote(d, open.id)); setOpenId(null); setConfirmDelete(false); flush(); }}>Delete</button>
              <button className="hub-btn" data-testid="note-delete-no" onClick={() => setConfirmDelete(false)}>Keep</button>
            </>
          ) : (
            <button className="hub-btn" data-testid="note-delete" onClick={() => setConfirmDelete(true)}>Delete</button>
          )}
        </div>
        <textarea
          ref={area}
          className="notes-area"
          data-testid="note-text"
          aria-label="Note text"
          value={open.body}
          maxLength={MAX_BODY}
          placeholder="Write something…"
          spellCheck
          onChange={(e) => {
            const before = open.body;
            update((d) => setBody(d, open.id, e.target.value, Date.now()));
            if (e.target.value.length > before.length && e.target.value !== before) setMessage(null);
          }}
        />
        <div className="notes-foot">
          <span data-testid="note-meta">Edited {ago(open.updatedAt, now)}</span>
          {left <= 500 && <span className="notes-warn" data-testid="note-left">{left} characters left</span>}
          {error && <span className="notes-error" role="alert" data-testid="notes-error">{error}</span>}
        </div>
      </div>
    );
  }

  return (
    <div className="notes" data-size={size} data-view="list" data-testid="widget-notes">
      <div className="notes-bar">
        <input
          className="notes-search"
          type="search"
          data-testid="notes-search"
          aria-label="Search notes"
          placeholder="Search notes"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button className="hub-btn primary" data-testid="note-new" onClick={create}>New note</button>
      </div>
      {message && <div className="notes-warn" role="status" data-testid="notes-message">{message}</div>}
      {error && <div className="notes-error" role="alert" data-testid="notes-error">{error}</div>}
      {list.length === 0 ? (
        <div className="notes-empty" data-testid="notes-empty">{doc.notes.length === 0 ? 'No notes yet. Press New note to write one.' : 'No note matches that search.'}</div>
      ) : (
        <ul className="notes-list" data-testid="notes-list">
          {list.map((n) => (
            <li key={n.id}>
              <button className="notes-row" data-testid="note-row" data-id={n.id} data-pinned={n.pinned} onClick={() => { setConfirmDelete(false); setOpenId(n.id); }}>
                <span className="nr-title">{n.pinned && <span className="nr-pin" aria-label="Pinned">●</span>}{titleOf(n)}</span>
                <span className="nr-prev">{previewOf(n)}</span>
                <span className="nr-when">{ago(n.updatedAt, now)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
