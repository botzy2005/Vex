import { describe, expect, it } from 'vitest';
import {
  addDays, addEvent, addMonths, busyDays, byTime, daysBetween, daysInMonth, EMPTY_DOC, eventsOn, isLeap, isValidDate, isValidTime, longDate,
  MAX_EVENTS, monthGrid, msToMidnight, normalize, relativeDay, removeEvent, todayOf, updateEvent, upcoming, weekdayMon0, weekdayNames, type CalDoc,
} from '../src/hub/widgets/calendar/logic';

const add = (d: CalDoc, date: string, title: string, time: string | null = null): CalDoc => {
  const r = addEvent(d, { date, time, title });
  if ('error' in r) throw new Error(r.error);
  return r.doc;
};

describe('dates', () => {
  it('knows leap years and month lengths', () => {
    expect([1900, 2000, 2024, 2026, 2100].map(isLeap)).toEqual([false, true, true, false, false]);
    expect(daysInMonth(2024, 2)).toBe(29);
    expect(daysInMonth(2026, 2)).toBe(28);
    expect(daysInMonth(2026, 11)).toBe(30);
  });
  it('validates real dates only', () => {
    for (const ok of ['2026-11-23', '2024-02-29', '1900-01-01', '2200-12-31']) expect(isValidDate(ok), ok).toBe(true);
    for (const bad of ['2026-02-29', '2026-13-01', '2026-00-10', '2026-04-31', '26-11-23', '2026-1-1', '', null, 5, '1899-12-31', '2201-01-01']) expect(isValidDate(bad), String(bad)).toBe(false);
    expect(isValidTime('00:00') && isValidTime('23:59')).toBe(true);
    for (const bad of ['24:00', '9:30', '12:60', 'noon', null]) expect(isValidTime(bad)).toBe(false);
  });
  it('adds days across month, year and leap boundaries', () => {
    expect(addDays('2026-11-30', 1)).toBe('2026-12-01');
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
    expect(addDays('2024-02-28', 1)).toBe('2024-02-29');
    expect(addDays('2026-02-28', 1)).toBe('2026-03-01');
    expect(addDays('2026-03-01', -1)).toBe('2026-02-28');
    expect(daysBetween('2026-11-23', '2026-12-25')).toBe(32);
    expect(daysBetween('2026-12-25', '2026-11-23')).toBe(-32);
  });
  it('finds the weekday', () => {
    expect(weekdayMon0('2026-11-23')).toBe(0); // Monday
    expect(weekdayMon0('2026-11-29')).toBe(6); // Sunday
    expect(weekdayMon0('2000-01-01')).toBe(5); // Saturday
  });
  it('moves between months', () => {
    expect(addMonths(2026, 12, 1)).toEqual({ y: 2027, m: 1 });
    expect(addMonths(2026, 1, -1)).toEqual({ y: 2025, m: 12 });
    expect(addMonths(2026, 11, 14)).toEqual({ y: 2028, m: 1 });
  });
});

describe('month grid', () => {
  it('has 42 days starting on the chosen weekday and containing the whole month', () => {
    for (const ws of ['mon', 'sun'] as const) {
      const g = monthGrid(2026, 11, ws);
      expect(g).toHaveLength(42);
      expect(weekdayMon0(g[0].date)).toBe(ws === 'mon' ? 0 : 6);
      expect(g.filter((c) => c.inMonth)).toHaveLength(30);
      expect(g.filter((c) => c.inMonth)[0].date).toBe('2026-11-01');
    }
  });
  it('starts November 2026 (1 November is a Sunday) on 26 October with Monday first and on 1 November with Sunday first', () => {
    expect(monthGrid(2026, 11, 'mon')[0].date).toBe('2026-10-26');
    expect(monthGrid(2026, 11, 'sun')[0].date).toBe('2026-11-01');
    expect(monthGrid(2026, 2, 'mon')[0].date).toBe('2026-01-26');
  });
  it('handles leap February and a month that starts on the first column', () => {
    expect(monthGrid(2024, 2, 'mon').filter((c) => c.inMonth)).toHaveLength(29);
    expect(monthGrid(2026, 6, 'mon')[0].date).toBe('2026-06-01'); // 1 June 2026 is a Monday
  });
  it('names the weekday columns', () => {
    expect(weekdayNames('mon')[0]).toBe('Mon');
    expect(weekdayNames('sun')).toEqual(['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']);
  });
});

describe('events', () => {
  it('adds, edits and removes, with fresh ids never reused', () => {
    let d = add(EMPTY_DOC, '2026-11-23', 'Dentist', '09:30');
    d = add(d, '2026-11-23', 'Birthday');
    expect(d.events.map((e) => e.id)).toEqual(['e1', 'e2']);
    const u = updateEvent(d, 'e1', { date: '2026-11-24', time: null, title: ' Dentist (moved) ', note: 'bring card' });
    expect('doc' in u && u.doc.events[0]).toMatchObject({ date: '2026-11-24', time: null, title: 'Dentist (moved)', note: 'bring card' });
    expect(updateEvent(d, 'zzz', { date: '2026-11-24', time: null, title: 'x' })).toEqual({ error: 'That event no longer exists.' });
    d = removeEvent(d, 'e2');
    expect(add(d, '2026-11-25', 'Later').events.at(-1)!.id).toBe('e3');
    expect(removeEvent(d, 'nope')).toBe(d);
  });
  it('refuses bad input with a reason', () => {
    expect(addEvent(EMPTY_DOC, { date: '2026-11-23', time: null, title: '   ' })).toEqual({ error: 'Give the event a title.' });
    expect(addEvent(EMPTY_DOC, { date: '2026-02-30', time: null, title: 'x' })).toEqual({ error: 'Choose a valid date.' });
    expect('error' in addEvent(EMPTY_DOC, { date: '2026-11-23', time: '25:00', title: 'x' })).toBe(true);
  });
  it('caps the number of events and the text lengths', () => {
    let d = EMPTY_DOC;
    for (let i = 0; i < MAX_EVENTS; i++) d = add(d, '2026-11-23', `e${i}`);
    expect('error' in addEvent(d, { date: '2026-11-23', time: null, title: 'one more' })).toBe(true);
    const r = addEvent(EMPTY_DOC, { date: '2026-11-23', time: null, title: 'x'.repeat(200), note: 'n'.repeat(900) });
    expect('doc' in r && r.doc.events[0].title).toHaveLength(80);
    expect('doc' in r && r.doc.events[0].note).toHaveLength(300);
  });
  it('orders a day: all-day first, then by time, then title', () => {
    let d = add(EMPTY_DOC, '2026-11-23', 'Lunch', '12:30');
    d = add(d, '2026-11-23', 'Standup', '09:00');
    d = add(d, '2026-11-23', 'Zebra day');
    d = add(d, '2026-11-23', 'Anniversary');
    d = add(d, '2026-11-24', 'Other day');
    expect(eventsOn(d.events, '2026-11-23').map((e) => e.title)).toEqual(['Anniversary', 'Zebra day', 'Standup', 'Lunch']);
    expect(byTime(d.events[0], d.events[1])).toBeGreaterThan(0);
  });
  it('lists what is coming up, including today, in order', () => {
    let d = add(EMPTY_DOC, '2026-11-22', 'Yesterday');
    d = add(d, '2026-11-30', 'Next Monday', '10:00');
    d = add(d, '2026-11-23', 'Today late', '18:00');
    d = add(d, '2026-11-23', 'Today early', '08:00');
    d = add(d, '2026-12-01', 'Outside the week');
    expect(upcoming(d.events, '2026-11-23', 8).map((e) => e.title)).toEqual(['Today early', 'Today late', 'Next Monday']);
    expect(upcoming(d.events, '2026-11-23', 7).map((e) => e.title)).toEqual(['Today early', 'Today late']);
  });
  it('marks the busy days of a month', () => {
    const d = add(add(add(EMPTY_DOC, '2026-11-03', 'a'), '2026-11-03', 'b'), '2026-12-01', 'c');
    expect([...busyDays(d.events, 2026, 11)]).toEqual(['2026-11-03']);
  });
  it('repairs the saved document', () => {
    expect(normalize(null)).toEqual(EMPTY_DOC);
    const d = normalize({
      weekStart: 'sun', next: 1,
      events: [
        { id: 'e5', date: '2026-11-23', time: '9:00', title: ' ok ', note: 7 },
        { id: 'e5', date: '2026-11-23', title: 'duplicate id' },
        { id: 'e6', date: '2026-02-30', title: 'bad date' },
        { id: 'e7', date: '2026-11-23', title: '   ' },
        { id: 'bad', date: '2026-11-23', title: 'bad id' },
        null,
      ],
    });
    expect(d.weekStart).toBe('sun');
    expect(d.events).toEqual([{ id: 'e5', date: '2026-11-23', time: null, title: 'ok', note: '' }]);
    expect(d.next).toBe(6);
    expect(normalize({ weekStart: 'fri' }).weekStart).toBe('mon');
  });
});

describe('labels and the clock', () => {
  it('describes days relative to today', () => {
    expect(relativeDay('2026-11-23', '2026-11-23')).toBe('Today');
    expect(relativeDay('2026-11-24', '2026-11-23')).toBe('Tomorrow');
    expect(relativeDay('2026-11-22', '2026-11-23')).toBe('Yesterday');
    expect(relativeDay('2026-11-28', '2026-11-23')).toBe('In 5 days');
    expect(relativeDay('2026-11-20', '2026-11-23')).toBe('3 days ago');
    expect(longDate('2026-11-23')).toBe('Mon 23 November 2026');
  });
  it('reads today from a clock value and counts down to midnight', () => {
    const noon = new Date(2026, 10, 23, 12, 0, 0).getTime();
    expect(todayOf(noon)).toBe('2026-11-23');
    expect(msToMidnight(noon)).toBe(12 * 3600_000 + 50);
    expect(todayOf(noon + msToMidnight(noon))).toBe('2026-11-24');
  });
});
