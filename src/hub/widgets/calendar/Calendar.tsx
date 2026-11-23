import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import type { WidgetProps } from '../../registry';
import { useWidgetData } from '../../useWidgetData';
import {
  addDays, addEvent, addMonths, busyDays, EMPTY_DOC, eventsOn, longDate, MAX_NOTE, MAX_TITLE, MONTHS, monthGrid, msToMidnight, normalize,
  relativeDay, removeEvent, todayOf, updateEvent, upcoming, weekdayNames, ymd, type CalDoc, type CalEvent,
} from './logic';
import './calendar.css';

interface Draft {
  id: string | null;
  date: string;
  time: string;
  title: string;
  note: string;
}

/** The date changes at local midnight, so "today" is recomputed then and nowhere else. */
function useToday(active: boolean): string {
  const [today, setToday] = useState(() => todayOf(Date.now()));
  useEffect(() => {
    if (!active) return;
    setToday(todayOf(Date.now()));
    let t: number;
    const tick = () => {
      setToday(todayOf(Date.now()));
      t = window.setTimeout(tick, msToMidnight(Date.now()));
    };
    t = window.setTimeout(tick, msToMidnight(Date.now()));
    return () => window.clearTimeout(t);
  }, [active]);
  return today;
}

export default function Calendar({ size, active }: WidgetProps) {
  const { value: doc, update, loaded, error } = useWidgetData<CalDoc>('calendar', EMPTY_DOC, normalize);
  const today = useToday(active);
  const [view, setView] = useState(() => ({ y: Number(today.slice(0, 4)), m: Number(today.slice(5, 7)) }));
  const [selected, setSelected] = useState(today);
  const [mode, setMode] = useState<'day' | 'week'>('day');
  const [draft, setDraft] = useState<Draft | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const titleRef = useRef<HTMLInputElement>(null);

  const grid = useMemo(() => monthGrid(view.y, view.m, doc.weekStart), [view, doc.weekStart]);
  const busy = useMemo(() => {
    // dots also for the leading and trailing days shown from the neighbouring months
    const set = new Set<string>();
    for (const c of grid) if (doc.events.some((e) => e.date === c.date)) set.add(c.date);
    return set;
  }, [grid, doc.events]);
  void busyDays;

  useEffect(() => {
    if (draft) titleRef.current?.focus();
  }, [draft?.id, draft === null]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!loaded) return <div className="cal" data-testid="widget-calendar" data-view="loading" />;

  const goto = (date: string) => {
    setSelected(date);
    setView({ y: Number(date.slice(0, 4)), m: Number(date.slice(5, 7)) });
  };
  const shiftMonth = (n: number) => {
    const v = addMonths(view.y, view.m, n);
    setView(v);
    setSelected(ymd(v.y, v.m, 1));
  };
  const onKey = (e: KeyboardEvent) => {
    const step: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
    if (e.key in step) {
      e.preventDefault();
      goto(addDays(selected, step[e.key]));
    } else if (e.key === 'PageUp' || e.key === 'PageDown') {
      e.preventDefault();
      shiftMonth(e.key === 'PageUp' ? -1 : 1);
    } else return;
    window.setTimeout(() => document.querySelector<HTMLElement>('[data-testid="cal-day"][data-selected="true"]')?.focus(), 0);
  };

  const startAdd = () => {
    setProblem(null);
    setDraft({ id: null, date: selected, time: '', title: '', note: '' });
  };
  const startEdit = (e: CalEvent) => {
    setProblem(null);
    setDraft({ id: e.id, date: e.date, time: e.time ?? '', title: e.title, note: e.note });
  };
  const save = () => {
    if (!draft) return;
    const input = { date: draft.date, time: draft.time === '' ? null : draft.time, title: draft.title, note: draft.note };
    const r = draft.id ? updateEvent(doc, draft.id, input) : addEvent(doc, input);
    if ('error' in r) return setProblem(r.error);
    update(r.doc);
    goto(draft.date);
    setDraft(null);
    setProblem(null);
  };

  const dayEvents = eventsOn(doc.events, selected);
  const weekEvents = upcoming(doc.events, today, 7);
  const list = mode === 'day' ? dayEvents : weekEvents;

  return (
    <div className="cal" data-size={size} data-testid="widget-calendar" data-view={draft ? 'edit' : 'month'}>
      <div className="cal-head">
        <button className="hub-btn" data-testid="cal-prev" aria-label="Previous month" onClick={() => shiftMonth(-1)}>‹</button>
        <b className="cal-title" data-testid="cal-title" aria-live="polite">{MONTHS[view.m - 1]} {view.y}</b>
        <button className="hub-btn" data-testid="cal-next" aria-label="Next month" onClick={() => shiftMonth(1)}>›</button>
        <span className="spacer" />
        <button className="hub-btn" data-testid="cal-today" onClick={() => goto(today)}>Today</button>
        <button className="hub-btn" data-testid="cal-weekstart" title="First day of the week" onClick={() => update({ ...doc, weekStart: doc.weekStart === 'mon' ? 'sun' : 'mon' })}>Week: {doc.weekStart === 'mon' ? 'Mon' : 'Sun'}</button>
      </div>
      <div className="cal-grid" role="grid" aria-label={`${MONTHS[view.m - 1]} ${view.y}`} onKeyDown={onKey}>
        {weekdayNames(doc.weekStart).map((w) => <span key={w} className="cal-wd" role="columnheader">{w}</span>)}
        {grid.map((c) => (
          <button
            key={c.date}
            role="gridcell"
            className="cal-day"
            data-testid="cal-day"
            data-date={c.date}
            data-in-month={c.inMonth}
            data-today={c.date === today}
            data-selected={c.date === selected}
            data-busy={busy.has(c.date)}
            aria-label={`${longDate(c.date)}${busy.has(c.date) ? ', has events' : ''}`}
            aria-selected={c.date === selected}
            tabIndex={c.date === selected ? 0 : -1}
            onClick={() => { setSelected(c.date); if (!c.inMonth) setView({ y: Number(c.date.slice(0, 4)), m: Number(c.date.slice(5, 7)) }); }}
            onDoubleClick={() => { setSelected(c.date); setDraft({ id: null, date: c.date, time: '', title: '', note: '' }); setProblem(null); }}
          >
            {Number(c.date.slice(8))}
          </button>
        ))}
      </div>
      {draft ? (
        <form className="cal-edit" data-testid="cal-form" onSubmit={(e) => { e.preventDefault(); save(); }}>
          <input ref={titleRef} className="cal-in" data-testid="cal-title-in" aria-label="Title" placeholder="Title" maxLength={MAX_TITLE} value={draft.title} onChange={(e) => setDraft({ ...draft, title: e.target.value })} />
          <div className="cal-row">
            <input className="cal-in" type="date" data-testid="cal-date-in" aria-label="Date" value={draft.date} onChange={(e) => setDraft({ ...draft, date: e.target.value })} />
            <input className="cal-in" type="time" data-testid="cal-time-in" aria-label="Time (leave empty for all day)" value={draft.time} onChange={(e) => setDraft({ ...draft, time: e.target.value })} />
          </div>
          <input className="cal-in" data-testid="cal-note-in" aria-label="Note" placeholder="Note (optional)" maxLength={MAX_NOTE} value={draft.note} onChange={(e) => setDraft({ ...draft, note: e.target.value })} />
          {problem && <div className="cal-problem" role="alert" data-testid="cal-problem">{problem}</div>}
          <div className="cal-row">
            <button className="hub-btn primary" type="submit" data-testid="cal-save">{draft.id ? 'Save' : 'Add event'}</button>
            <button className="hub-btn" type="button" data-testid="cal-cancel" onClick={() => { setDraft(null); setProblem(null); }}>Cancel</button>
          </div>
        </form>
      ) : (
        <div className="cal-agenda">
          <div className="cal-agenda-head">
            <span className="cal-seg" role="group" aria-label="What to list">
              <button className="chip" data-testid="cal-mode-day" aria-pressed={mode === 'day'} onClick={() => setMode('day')}>{relativeDay(selected, today) === 'Today' ? 'Today' : longDate(selected).slice(0, -5)}</button>
              <button className="chip" data-testid="cal-mode-week" aria-pressed={mode === 'week'} onClick={() => setMode('week')}>Next 7 days</button>
            </span>
            <span className="spacer" />
            <button className="hub-btn primary" data-testid="cal-add" onClick={startAdd}>Add event</button>
          </div>
          {error && <div className="cal-problem" role="alert" data-testid="cal-error">{error}</div>}
          {problem && <div className="cal-problem" role="alert" data-testid="cal-problem">{problem}</div>}
          {list.length === 0 ? (
            <div className="cal-none" data-testid="cal-none">{mode === 'day' ? 'Nothing on this day.' : 'Nothing in the next 7 days.'}</div>
          ) : (
            <ul className="cal-events" data-testid="cal-events">
              {list.map((e) => (
                <li key={e.id} className="cal-event" data-testid="cal-event" data-id={e.id}>
                  <span className="ce-when">{mode === 'week' ? `${relativeDay(e.date, today)}${e.time ? ` ${e.time}` : ''}` : e.time ?? 'All day'}</span>
                  <span className="ce-title" title={e.note || undefined}><b>{e.title}</b>{e.note && <small>{e.note}</small>}</span>
                  <button className="hub-btn" data-testid="cal-edit" aria-label={`Edit ${e.title}`} onClick={() => startEdit(e)}>Edit</button>
                  <button className="hub-btn danger" data-testid="cal-delete" aria-label={`Delete ${e.title}`} onClick={() => update((d) => removeEvent(d, e.id))}>✕</button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
