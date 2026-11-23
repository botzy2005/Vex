/** Notes rules. Pure, so ordering, search, limits and repair are unit-tested without a browser. */

export interface Note {
  id: string;
  body: string;
  pinned: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface NotesDoc {
  version: 1;
  /** Next id to hand out ("n1", "n2", ...). Never reused, so a deleted note's id cannot come back. */
  next: number;
  notes: Note[];
}

export const MAX_NOTES = 60;
export const MAX_BODY = 8000;
/** Conservative bound on the saved document (the Hub store caps one widget's data at 512 KB). */
export const MAX_DOC_BYTES = 400_000;

export const EMPTY_DOC: NotesDoc = { version: 1, next: 1, notes: [] };

const time = (v: unknown, fallback: number): number => (typeof v === 'number' && Number.isFinite(v) && v >= 0 ? Math.floor(v) : fallback);

/** Repair whatever was stored (older version, hand edit, garbage) into a usable document. */
export function normalize(raw: unknown): NotesDoc {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const seen = new Set<string>();
  const notes: Note[] = [];
  if (Array.isArray(r.notes)) {
    for (const n of r.notes) {
      if (!n || typeof n !== 'object') continue;
      const o = n as Record<string, unknown>;
      if (typeof o.id !== 'string' || !/^n\d{1,9}$/.test(o.id) || seen.has(o.id) || typeof o.body !== 'string') continue;
      seen.add(o.id);
      const updatedAt = time(o.updatedAt, 0);
      notes.push({ id: o.id, body: o.body.slice(0, MAX_BODY), pinned: o.pinned === true, createdAt: time(o.createdAt, updatedAt), updatedAt });
      if (notes.length >= MAX_NOTES) break;
    }
  }
  const highest = notes.reduce((m, n) => Math.max(m, Number(n.id.slice(1))), 0);
  const next = typeof r.next === 'number' && Number.isInteger(r.next) && r.next > highest ? r.next : highest + 1;
  return { version: 1, next, notes };
}

export function sizeBytes(doc: NotesDoc): number {
  return new TextEncoder().encode(JSON.stringify(doc)).length;
}

export const fits = (doc: NotesDoc): boolean => sizeBytes(doc) <= MAX_DOC_BYTES;

export function canAdd(doc: NotesDoc): { ok: true } | { ok: false; reason: string } {
  if (doc.notes.length >= MAX_NOTES) return { ok: false, reason: `You can keep up to ${MAX_NOTES} notes. Delete one to add another.` };
  if (!fits(doc)) return { ok: false, reason: 'Notes storage is full. Delete a note to add another.' };
  return { ok: true };
}

export function addNote(doc: NotesDoc, now: number): { doc: NotesDoc; id: string } {
  const id = `n${doc.next}`;
  const note: Note = { id, body: '', pinned: false, createdAt: now, updatedAt: now };
  return { doc: { ...doc, next: doc.next + 1, notes: [note, ...doc.notes] }, id };
}

/** Replace a note's text. Returns the same document when nothing changes or the result would not fit. */
export function setBody(doc: NotesDoc, id: string, body: string, now: number): NotesDoc {
  const cur = doc.notes.find((n) => n.id === id);
  const text = body.slice(0, MAX_BODY);
  if (!cur || cur.body === text) return doc;
  const next = { ...doc, notes: doc.notes.map((n) => (n.id === id ? { ...n, body: text, updatedAt: now } : n)) };
  return fits(next) || text.length < cur.body.length ? next : doc;
}

export function togglePin(doc: NotesDoc, id: string): NotesDoc {
  return { ...doc, notes: doc.notes.map((n) => (n.id === id ? { ...n, pinned: !n.pinned } : n)) };
}

export function removeNote(doc: NotesDoc, id: string): NotesDoc {
  return doc.notes.some((n) => n.id === id) ? { ...doc, notes: doc.notes.filter((n) => n.id !== id) } : doc;
}

/** First non-empty line, trimmed and shortened. */
export function titleOf(n: Pick<Note, 'body'>): string {
  const line = n.body.split('\n').map((l) => l.trim()).find((l) => l !== '') ?? '';
  return line === '' ? 'Untitled note' : line.length > 60 ? `${line.slice(0, 59)}…` : line;
}

/** The text after the title line, flattened to one short line. */
export function previewOf(n: Pick<Note, 'body'>): string {
  const lines = n.body.split('\n').map((l) => l.trim()).filter((l) => l !== '');
  return lines.slice(1).join(' · ').slice(0, 90);
}

/** Pinned first, then most recently edited first. */
export function ordered(notes: Note[]): Note[] {
  return [...notes].sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt);
}

/** Case-insensitive, every word must appear somewhere in the note. */
export function search(notes: Note[], query: string): Note[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const list = ordered(notes);
  return words.length === 0 ? list : list.filter((n) => words.every((w) => n.body.toLowerCase().includes(w)));
}

/** "just now", "5 min ago", "3 h ago", "yesterday", or a short date. */
export function ago(then: number, now: number): string {
  const s = Math.max(0, Math.floor((now - then) / 1000));
  if (s < 60) return 'just now';
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  if (s < 172800) return 'yesterday';
  return new Date(then).toLocaleDateString([], { day: 'numeric', month: 'short' });
}
