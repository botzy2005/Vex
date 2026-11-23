import { useEffect, useMemo, useState } from 'react';
import type { WidgetProps } from '../../registry';
import { MoonIcon, SunIcon } from '../../icons';
import { useWidgetData } from '../../useWidgetData';
import {
  addClock, DEFAULT_DOC, dayLabel, diffLabel, isDaytime, moveClock, msToNextMinute, normalize, removeClock, searchCities, systemZone,
  timeIn, type ClockDoc,
} from './logic';
import { CITIES } from './cities';
import './worldclock.css';

/** Redraws once a minute, on the minute, and only while the Hub is visible. */
function useMinuteNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    let t: number;
    const tick = () => {
      setNow(Date.now());
      t = window.setTimeout(tick, msToNextMinute(Date.now()));
    };
    t = window.setTimeout(tick, msToNextMinute(Date.now()));
    return () => window.clearTimeout(t);
  }, [active]);
  return now;
}

const FORMATS: Array<{ v: boolean | null; label: string }> = [
  { v: null, label: 'System' },
  { v: false, label: '24 h' },
  { v: true, label: '12 h' },
];

export default function WorldClock({ size, active }: WidgetProps) {
  const { value: doc, update, loaded, error } = useWidgetData<ClockDoc>('worldclock', DEFAULT_DOC, normalize);
  const now = useMinuteNow(active);
  const local = useMemo(systemZone, []);
  const [edit, setEdit] = useState(false);
  const [adding, setAdding] = useState(false);
  const [query, setQuery] = useState('');
  const results = useMemo(() => searchCities(query, doc.clocks.map((c) => c.zone)), [query, doc.clocks]);
  const format = FORMATS.find((f) => f.v === doc.hour12) ?? FORMATS[0];
  const full = doc.clocks.length >= 12;

  if (!loaded) return <div className="wc" data-testid="widget-worldclock" data-view="loading" />;

  const row = (zone: string, label: string, isLocal: boolean, i: number) => {
    const day = dayLabel(zone, local, now);
    const diff = diffLabel(zone, local, now);
    return (
      <li className="wc-row" key={isLocal ? 'local' : zone} data-testid="wc-row" data-zone={isLocal ? 'local' : zone} data-day={isDaytime(zone, now) ? 'day' : 'night'}>
        <span className="wc-glyph" aria-hidden="true">{isDaytime(zone, now) ? <SunIcon width={15} height={15} /> : <MoonIcon width={15} height={15} />}</span>
        <span className="wc-name">
          <b data-testid="wc-label">{label}</b>
          <small data-testid="wc-sub">{isLocal ? 'Your time zone' : `${day} · ${diff}`}</small>
        </span>
        <span className="wc-time" data-testid="wc-time">{timeIn(zone, now, doc.hour12)}</span>
        {edit && !isLocal && (
          <span className="wc-edit">
            <button className="hub-btn" data-testid="wc-up" aria-label={`Move ${label} up`} disabled={i === 0} onClick={() => update((d) => moveClock(d, zone, -1))}>↑</button>
            <button className="hub-btn" data-testid="wc-down" aria-label={`Move ${label} down`} disabled={i === doc.clocks.length - 1} onClick={() => update((d) => moveClock(d, zone, 1))}>↓</button>
            <button className="hub-btn danger" data-testid="wc-remove" aria-label={`Remove ${label}`} onClick={() => update((d) => removeClock(d, zone))}>✕</button>
          </span>
        )}
      </li>
    );
  };

  return (
    <div className="wc" data-size={size} data-view={adding ? 'add' : 'clocks'} data-testid="widget-worldclock">
      <div className="wc-bar">
        <button className="hub-btn primary" data-testid="wc-add" disabled={full && !adding} title={full ? 'You can keep up to 12 clocks' : undefined} onClick={() => { setAdding((a) => !a); setQuery(''); }}>
          {adding ? 'Done' : 'Add city'}
        </button>
        <span className="spacer" />
        <button className="hub-btn" data-testid="wc-format" title="Switch between the system setting, 24-hour and 12-hour" onClick={() => update((d) => ({ ...d, hour12: FORMATS[(FORMATS.indexOf(format) + 1) % FORMATS.length].v }))}>{format.label}</button>
        <button className="hub-btn" data-testid="wc-edit" aria-pressed={edit} onClick={() => setEdit((e) => !e)}>{edit ? 'Finish' : 'Edit'}</button>
      </div>
      {error && <div className="wc-error" role="alert" data-testid="wc-error">{error}</div>}
      {adding && (
        <div className="wc-add" data-testid="wc-adding">
          <input className="wc-search" type="search" autoFocus data-testid="wc-search" aria-label="Search for a city" placeholder={`Search ${CITIES.length} cities`} value={query} onChange={(e) => setQuery(e.target.value)} />
          {query.trim() !== '' && results.length === 0 && <div className="wc-none" data-testid="wc-none">No city matches that.</div>}
          <ul className="wc-results">
            {results.map((c) => (
              <li key={c.zone}>
                <button className="wc-result" data-testid="wc-result" data-zone={c.zone} onClick={() => { update((d) => addClock(d, c)); setQuery(''); }}>
                  <span>{c.name}</span><small>{c.country}</small>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      <ul className="wc-list" data-testid="wc-list">
        {row(local, 'Local time', true, -1)}
        {doc.clocks.map((c, i) => row(c.zone, c.label, false, i))}
        {doc.clocks.length === 0 && <li className="wc-empty" data-testid="wc-empty">No other clocks yet. Press Add city.</li>}
      </ul>
    </div>
  );
}
