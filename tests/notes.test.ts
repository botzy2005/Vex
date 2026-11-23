import { describe, expect, it } from 'vitest';
import {
  addNote, ago, canAdd, EMPTY_DOC, fits, MAX_BODY, MAX_DOC_BYTES, MAX_NOTES, normalize, ordered, previewOf, removeNote, search,
  setBody, titleOf, togglePin, type NotesDoc,
} from '../src/hub/widgets/notes/logic';

const doc = (): NotesDoc => EMPTY_DOC;

describe('notes document', () => {
  it('survives garbage and drops bad entries', () => {
    expect(normalize(null)).toEqual(EMPTY_DOC);
    expect(normalize('x')).toEqual(EMPTY_DOC);
    const d = normalize({
      next: 2,
      notes: [
        { id: 'n1', body: 'ok', pinned: true, createdAt: 5, updatedAt: 9 },
        { id: 'n1', body: 'duplicate id', updatedAt: 1 },
        { id: 'bad id', body: 'x' },
        { id: 'n3', body: 42 },
        null,
        { id: 'n7', body: 'a'.repeat(MAX_BODY + 50), updatedAt: -4 },
      ],
    });
    expect(d.notes.map((n) => n.id)).toEqual(['n1', 'n7']);
    expect(d.notes[1].body).toHaveLength(MAX_BODY);
    expect(d.notes[1].updatedAt).toBe(0);
    expect(d.next).toBe(8); // never at or below an existing id
  });
  it('keeps a valid document unchanged', () => {
    let d = addNote(doc(), 100).doc;
    d = setBody(d, 'n1', 'Shopping\nmilk', 200);
    expect(normalize(JSON.parse(JSON.stringify(d)))).toEqual(d);
  });
  it('caps the number of notes on load', () => {
    const notes = Array.from({ length: MAX_NOTES + 10 }, (_, i) => ({ id: `n${i + 1}`, body: 'x' }));
    expect(normalize({ notes }).notes).toHaveLength(MAX_NOTES);
  });
});

describe('notes operations', () => {
  it('adds a note at the top with a fresh id that is never reused', () => {
    const a = addNote(doc(), 1);
    const b = addNote(a.doc, 2);
    expect([a.id, b.id]).toEqual(['n1', 'n2']);
    expect(b.doc.notes[0].id).toBe('n2');
    const c = addNote(removeNote(b.doc, 'n2'), 3);
    expect(c.id).toBe('n3');
  });
  it('edits, bounds the text and stamps the time only on a real change', () => {
    const { doc: d } = addNote(doc(), 1);
    const e = setBody(d, 'n1', 'hello', 50);
    expect(e.notes[0]).toMatchObject({ body: 'hello', updatedAt: 50 });
    expect(setBody(e, 'n1', 'hello', 99)).toBe(e);
    expect(setBody(e, 'nope', 'x', 99)).toBe(e);
    expect(setBody(e, 'n1', 'z'.repeat(MAX_BODY + 5), 60).notes[0].body).toHaveLength(MAX_BODY);
  });
  it('pins and unpins, and deletes', () => {
    const d = addNote(addNote(doc(), 1).doc, 2).doc;
    expect(togglePin(d, 'n1').notes.find((n) => n.id === 'n1')!.pinned).toBe(true);
    expect(togglePin(togglePin(d, 'n1'), 'n1')).toEqual(d);
    expect(removeNote(d, 'n1').notes.map((n) => n.id)).toEqual(['n2']);
    expect(removeNote(d, 'zzz')).toBe(d);
  });
  it('refuses new notes past the count limit and past the storage limit', () => {
    let d = doc();
    for (let i = 0; i < MAX_NOTES; i++) d = addNote(d, i).doc;
    expect(canAdd(d)).toMatchObject({ ok: false });
    expect(canAdd(removeNote(d, 'n1'))).toEqual({ ok: true });
  });
  it('never lets edits grow the stored document past the limit', () => {
    let d = doc();
    for (let i = 0; i < 50; i++) d = setBody(addNote(d, i).doc, `n${i + 1}`, 'é'.repeat(MAX_BODY), i); // 2 bytes each
    expect(fits(d)).toBe(true);
    expect(d.notes.some((n) => n.body.length < MAX_BODY)).toBe(true); // later edits were refused once it was full
    expect(MAX_DOC_BYTES).toBeLessThan(512 * 1024);
  });
  it('allows shrinking a document that is already over the limit (e.g. edited by hand)', () => {
    const big: NotesDoc = { version: 1, next: 51, notes: Array.from({ length: 50 }, (_, i) => ({ id: `n${i + 1}`, body: 'é'.repeat(MAX_BODY), pinned: false, createdAt: 0, updatedAt: 0 })) };
    expect(fits(big)).toBe(false);
    expect(canAdd(big)).toMatchObject({ ok: false });
    expect(setBody(big, 'n1', 'é'.repeat(MAX_BODY) + 'x', 5)).toBe(big);
    expect(setBody(big, 'n1', 'short', 5).notes[0].body).toBe('short');
  });
});

describe('notes presentation', () => {
  it('derives title and preview from the text', () => {
    expect(titleOf({ body: '\n  Plan the week \nMon: gym\n\nTue: read' })).toBe('Plan the week');
    expect(previewOf({ body: '\n  Plan the week \nMon: gym\n\nTue: read' })).toBe('Mon: gym · Tue: read');
    expect(titleOf({ body: '   \n ' })).toBe('Untitled note');
    expect(titleOf({ body: 'x'.repeat(100) })).toHaveLength(60);
  });
  it('orders pinned first, then newest edit first', () => {
    let d = doc();
    for (const t of [10, 30, 20]) d = setBody(addNote(d, t).doc, `n${d.next}`, `note ${t}`, t);
    d = togglePin(d, 'n1');
    expect(ordered(d.notes).map((n) => n.body)).toEqual(['note 10', 'note 30', 'note 20']);
  });
  it('searches every word, case-insensitively, anywhere in the note', () => {
    let d = doc();
    d = setBody(addNote(d, 1).doc, 'n1', 'Buy Milk and eggs', 1);
    d = setBody(addNote(d, 2).doc, 'n2', 'Call the dentist\nabout milk allergy', 2);
    d = setBody(addNote(d, 3).doc, 'n3', 'Ideas', 3);
    expect(search(d.notes, 'milk').map((n) => n.id)).toEqual(['n2', 'n1']);
    expect(search(d.notes, 'MILK dentist').map((n) => n.id)).toEqual(['n2']);
    expect(search(d.notes, 'zzz')).toEqual([]);
    expect(search(d.notes, '   ')).toHaveLength(3);
  });
  it('describes recent edits in words', () => {
    const now = 10 * 86400_000;
    expect(ago(now - 5_000, now)).toBe('just now');
    expect(ago(now - 5 * 60_000, now)).toBe('5 min ago');
    expect(ago(now - 3 * 3600_000, now)).toBe('3 h ago');
    expect(ago(now - 30 * 3600_000, now)).toBe('yesterday');
    expect(ago(now + 5000, now)).toBe('just now');
  });
});
