import { useCallback, useEffect, useState } from 'react';
import { useBackend } from '../../../backend/hooks';
import type { WidgetProps } from '../../registry';
import { useTimers } from '../../useHub';
import { useWidgetData } from '../../useWidgetData';
import { formatRemaining, remainingMs } from '../../timeFormat';
import { cleanLabel, DEFAULT_DOC, describeLength, MAX_TIMERS, mine, normalize, OWNER, parseDuration, PRESETS, soonest, type CountdownDoc } from './logic';
import './countdown.css';

export default function Countdown({ size, active }: WidgetProps) {
  const backend = useBackend();
  const { value: doc, update, loaded } = useWidgetData<CountdownDoc>('countdown', DEFAULT_DOC, normalize);
  const { timers, now } = useTimers(active);
  const list = mine(timers);
  const [label, setLabel] = useState('');
  const [h, setH] = useState('');
  const [m, setM] = useState('');
  const [s, setS] = useState('');
  const [problem, setProblem] = useState<string | null>(null);
  const full = list.length >= MAX_TIMERS;

  const fail = useCallback((e: unknown) => setProblem(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)), []);
  const run = (f: () => Promise<unknown>) => {
    setProblem(null);
    f().catch(fail);
  };

  const create = (ms: number, name: string) =>
    run(async () => {
      if (full) throw `You can keep up to ${MAX_TIMERS} countdowns. Delete one to add another.`;
      const id = await backend.timerCreate(OWNER, cleanLabel(name) || describeLength(ms), ms);
      await backend.timerStart(id);
    });

  const startCustom = () => {
    const r = parseDuration(h, m, s);
    if (!r.ok) return setProblem(r.reason);
    create(r.ms, label);
    setLabel('');
    setH('');
    setM('');
    setS('');
  };

  // The island shows the countdown that ends first (in memory only; cleared when this card goes away).
  const next = soonest(timers);
  const nextId = next?.id ?? null;
  const nextEnds = next?.endsAt ?? 0;
  const nextLabel = next?.label ?? '';
  useEffect(() => {
    if (nextId === null) {
      void backend.hubSetStatus(OWNER, null).catch(() => {});
      return;
    }
    const detail = `ends ${new Date(nextEnds).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
    void backend.hubSetStatus(OWNER, { label: nextLabel || 'Countdown', detail, priority: 1 }).catch(() => {});
  }, [backend, nextId, nextEnds, nextLabel]);
  useEffect(() => () => void backend.hubSetStatus(OWNER, null).catch(() => {}), [backend]);

  if (!loaded) return <div className="cd" data-testid="widget-countdown" data-view="loading" />;

  return (
    <div className="cd" data-size={size} data-testid="widget-countdown">
      <div className="cd-presets" role="group" aria-label="Quick countdowns">
        {PRESETS.map((p) => (
          <button key={p.ms} className="chip" data-testid="cd-preset" data-ms={p.ms} disabled={full} onClick={() => create(p.ms, '')}>{p.label}</button>
        ))}
      </div>
      <form className="cd-custom" data-testid="cd-custom" onSubmit={(e) => { e.preventDefault(); startCustom(); }}>
        <input className="cd-label" data-testid="cd-label" aria-label="Label" placeholder="Label (optional)" maxLength={40} value={label} onChange={(e) => setLabel(e.target.value)} />
        <input className="cd-num" data-testid="cd-h" aria-label="Hours" inputMode="numeric" placeholder="h" value={h} onChange={(e) => setH(e.target.value)} />
        <input className="cd-num" data-testid="cd-m" aria-label="Minutes" inputMode="numeric" placeholder="min" value={m} onChange={(e) => setM(e.target.value)} />
        <input className="cd-num" data-testid="cd-s" aria-label="Seconds" inputMode="numeric" placeholder="s" value={s} onChange={(e) => setS(e.target.value)} />
        <button className="hub-btn primary" type="submit" data-testid="cd-start" disabled={full}>Start</button>
      </form>
      {problem && <div className="cd-problem" role="alert" data-testid="cd-problem">{problem}</div>}
      {list.length === 0 ? (
        <div className="cd-empty" data-testid="cd-empty">No countdowns. Pick a quick one above, or set your own time.</div>
      ) : (
        <ul className="cd-list" data-testid="cd-list">
          {list.map((t) => {
            const left = remainingMs(t, now);
            return (
              <li className="cd-row" key={t.id} data-testid="cd-row" data-id={t.id} data-state={t.state}>
                <span className="cd-name"><b data-testid="cd-name">{t.label}</b><small>{t.state === 'finished' ? "Time's up" : t.state === 'paused' ? 'Paused' : describeLength(t.durationMs)}</small></span>
                <span className="cd-time" data-testid="cd-time" role="timer" aria-label={`${t.label}, ${formatRemaining(left)} left`}>{t.state === 'finished' ? '0:00' : formatRemaining(left)}</span>
                <span className="cd-actions">
                  {t.state === 'finished' ? (
                    <button className="hub-btn" data-testid="cd-restart" onClick={() => run(async () => { await backend.timerReset(t.id); await backend.timerStart(t.id); })}>Restart</button>
                  ) : t.state === 'running' ? (
                    <button className="hub-btn" data-testid="cd-pause" onClick={() => run(() => backend.timerPause(t.id))}>Pause</button>
                  ) : (
                    <button className="hub-btn" data-testid="cd-resume" onClick={() => run(() => backend.timerStart(t.id))}>{t.state === 'paused' ? 'Resume' : 'Start'}</button>
                  )}
                  <button className="hub-btn" data-testid="cd-reset" disabled={t.state === 'idle'} onClick={() => run(() => backend.timerReset(t.id))}>Reset</button>
                  <button className="hub-btn danger" data-testid="cd-delete" aria-label={`Delete ${t.label}`} onClick={() => run(() => backend.timerDelete(t.id))}>✕</button>
                </span>
              </li>
            );
          })}
        </ul>
      )}
      <label className="cd-sound"><input type="checkbox" data-testid="cd-sound" checked={doc.sound} onChange={(e) => update({ ...doc, sound: e.target.checked })} /> Play a chime when a countdown ends</label>
    </div>
  );
}
