import { useCallback, useEffect, useRef, useState } from 'react';
import { useBackend } from '../../../backend/hooks';
import type { WidgetProps } from '../../registry';
import { useTimers } from '../../useHub';
import { useWidgetData } from '../../useWidgetData';
import { formatRemaining, remainingMs } from '../../timeFormat';
import {
  applyFinish, applySkip, DEFAULT_DATA, LIMITS, normalize, normalizeSettings, PHASE_LABEL, phaseMs, progress, shouldAutoStart,
  todayCount, type Phase, type PomodoroData, type PomodoroSettings,
} from './logic';
import './pomodoro.css';

const OWNER = 'pomodoro';
const R = 44;
const CIRC = 2 * Math.PI * R;

function NumberField({ label, value, min, max, unit, testId, onCommit }: { label: string; value: number; min: number; max: number; unit?: string; testId: string; onCommit(v: number): void }) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const commit = () => {
    const n = Number(draft);
    if (draft.trim() === '' || !Number.isFinite(n)) return setDraft(String(value));
    onCommit(Math.max(min, Math.min(max, Math.round(n))));
  };
  return (
    <label className="pomo-field">
      <span>{label}</span>
      <span className="pomo-input">
        <input type="number" inputMode="numeric" min={min} max={max} value={draft} data-testid={testId} aria-label={label}
          onChange={(e) => setDraft(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === 'Enter' && (e.currentTarget.blur())} />
        {unit && <em>{unit}</em>}
      </span>
    </label>
  );
}

export default function Pomodoro({ size, active }: WidgetProps) {
  const backend = useBackend();
  const { value: data, update, loaded } = useWidgetData<PomodoroData>('pomodoro', DEFAULT_DATA, normalize);
  const { timers, now } = useTimers(active);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const busy = useRef(false);
  const dataRef = useRef(data);
  dataRef.current = data;

  const timer = timers.find((t) => t.id === data.timerId && t.owner === OWNER);
  const fail = useCallback((e: unknown) => setError(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)), []);

  /** Give the document a fresh timer for its phase (and drop the old one). */
  const swap = useCallback(
    async (next: PomodoroData, start: boolean) => {
      const id = await backend.timerCreate(OWNER, PHASE_LABEL[next.phase], phaseMs(next.phase, next.settings));
      const old = dataRef.current.timerId;
      // Point the document at the new timer first, so nothing ever sees a phase without its timer.
      update({ ...next, timerId: id });
      dataRef.current = { ...next, timerId: id };
      if (old !== null) await backend.timerDelete(old).catch(() => {});
      if (start) await backend.timerStart(id);
    },
    [backend, update],
  );

  // Make sure the current phase has a timer (first run, or the stored one is gone).
  useEffect(() => {
    if (!loaded || busy.current || timer) return;
    busy.current = true;
    void (async () => {
      const list = (await backend.timersList()).filter((t) => t.owner === OWNER);
      const d = dataRef.current;
      const keep = list.find((t) => t.id === d.timerId) ?? list[0];
      if (keep) {
        for (const t of list) if (t.id !== keep.id) await backend.timerDelete(t.id).catch(() => {});
        if (keep.state === 'idle' && keep.durationMs !== phaseMs(d.phase, d.settings)) await backend.timerSetDuration(keep.id, phaseMs(d.phase, d.settings));
        if (keep.id !== d.timerId) update({ ...d, timerId: keep.id });
      } else {
        await swap(d, false);
      }
    })().catch(fail).finally(() => (busy.current = false));
  }, [loaded, timer, data.timerId, backend, swap, update, fail]);

  // A phase ended (live, or while the Hub was closed): count it once, move on, and optionally start the next one.
  useEffect(() => {
    if (!loaded || busy.current || !timer || timer.state !== 'finished') return;
    busy.current = true;
    const d = dataRef.current;
    const at = timer.at;
    const next = applyFinish(d, at);
    setNotice(`${PHASE_LABEL[d.phase]} finished`);
    void swap(next, shouldAutoStart(next, Date.now() - at)).catch(fail).finally(() => (busy.current = false));
  }, [loaded, timer, swap, fail]);

  // Tell the island what is running (in memory only; cleared when this card goes away).
  const running = timer?.state === 'running';
  const paused = timer?.state === 'paused';
  const endsAt = timer?.state === 'running' ? timer.endsAt : 0;
  useEffect(() => {
    if (!running && !paused) {
      void backend.hubSetStatus(OWNER, null).catch(() => {});
      return;
    }
    const detail = running ? `ends ${new Date(endsAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}` : 'paused';
    void backend.hubSetStatus(OWNER, { label: PHASE_LABEL[data.phase], detail, priority: 1 }).catch(() => {});
  }, [backend, running, paused, endsAt, data.phase]);
  useEffect(() => () => void backend.hubSetStatus(OWNER, null).catch(() => {}), [backend]);

  const act = (f: () => Promise<unknown>) => () => {
    setError(null);
    f().catch(fail);
  };

  const phase: Phase = data.phase;
  const total = timer?.durationMs ?? phaseMs(phase, data.settings);
  const left = timer ? remainingMs(timer, now) : total;
  const startPause = running
    ? act(() => backend.timerPause(timer!.id))
    : act(async () => {
        setNotice(null);
        await backend.timerStart(timer!.id);
      });
  const reset = act(() => backend.timerReset(timer!.id));
  const skip = () => {
    if (busy.current) return;
    busy.current = true;
    setError(null);
    setNotice(null);
    swap(applySkip(dataRef.current), false).catch(fail).finally(() => (busy.current = false));
  };

  const saveSettings = (patch: Partial<PomodoroSettings>) => {
    const settings = normalizeSettings({ ...data.settings, ...patch });
    const cycle = Math.min(data.cycle, settings.longEvery);
    update({ ...data, settings, cycle });
    // A timer that has not started yet picks up the new length at once; a running one keeps its own.
    if (timer && timer.state === 'idle') {
      const ms = phaseMs(phase, settings);
      if (ms !== timer.durationMs) backend.timerSetDuration(timer.id, ms).catch(fail);
    }
  };

  const today = todayCount(data.history, now);
  const dots = Array.from({ length: data.settings.longEvery }, (_, i) => i < data.cycle);

  return (
    <div className="pomo" data-size={size} data-phase={phase} data-state={timer?.state ?? 'loading'} data-testid="widget-pomodoro">
      {!showSettings && (
        <>
      <div className="pomo-main">
        <div className="pomo-ring">
          <svg viewBox="0 0 100 100" aria-hidden="true">
            <circle className="pomo-track" cx="50" cy="50" r={R} />
            <circle className="pomo-arc" cx="50" cy="50" r={R} strokeDasharray={CIRC} strokeDashoffset={CIRC * (1 - progress(total, left))} />
          </svg>
          <div className="pomo-time" role="timer" aria-label={`${PHASE_LABEL[phase]}, ${formatRemaining(left)} left`} data-testid="pomo-time">{formatRemaining(left)}</div>
        </div>
        <div className="pomo-side">
          <div className="pomo-phase" data-testid="pomo-phase">{PHASE_LABEL[phase]}{paused ? ' · paused' : ''}</div>
          <div className="pomo-dots" data-testid="pomo-dots" aria-label={`${data.cycle} of ${data.settings.longEvery} focus sessions done in this set`}>
            {dots.map((on, i) => <i key={i} data-on={on} />)}
          </div>
          <div className="pomo-actions">
            <button className="hub-btn primary" data-testid="pomo-start" disabled={!timer || timer.state === 'finished'} onClick={startPause}>
              {running ? 'Pause' : paused ? 'Resume' : 'Start'}
            </button>
            <button className="hub-btn" data-testid="pomo-reset" disabled={!timer || timer.state === 'idle'} onClick={reset} title="Back to the full length of this phase">Reset</button>
            <button className="hub-btn" data-testid="pomo-skip" disabled={!timer} onClick={skip} title="Move to the next phase without counting this one">Skip</button>
            <button className="hub-btn" data-testid="pomo-gear" aria-expanded={showSettings} aria-label="Pomodoro settings" onClick={() => setShowSettings((v) => !v)}>⚙</button>
          </div>
        </div>
      </div>
      <div className="pomo-foot">
        <span data-testid="pomo-today">Today: {today} focus {today === 1 ? 'session' : 'sessions'}</span>
        {notice && <span className="pomo-notice" data-testid="pomo-notice" role="status">{notice}</span>}
      </div>
      {error && <div className="pomo-error" role="alert" data-testid="pomo-error">{error}</div>}
        </>
      )}
      {showSettings && (
        <div className="pomo-settings" data-testid="pomo-settings">
          <button className="hub-btn pomo-done" data-testid="pomo-gear" aria-expanded="true" onClick={() => setShowSettings(false)}>Done</button>
          <NumberField label="Focus" unit="min" min={LIMITS.focusMin[0]} max={LIMITS.focusMin[1]} value={data.settings.focusMin} testId="pomo-focus-min" onCommit={(v) => saveSettings({ focusMin: v })} />
          <NumberField label="Short break" unit="min" min={LIMITS.shortMin[0]} max={LIMITS.shortMin[1]} value={data.settings.shortMin} testId="pomo-short-min" onCommit={(v) => saveSettings({ shortMin: v })} />
          <NumberField label="Long break" unit="min" min={LIMITS.longMin[0]} max={LIMITS.longMin[1]} value={data.settings.longMin} testId="pomo-long-min" onCommit={(v) => saveSettings({ longMin: v })} />
          <NumberField label="Long break after" unit="sessions" min={LIMITS.longEvery[0]} max={LIMITS.longEvery[1]} value={data.settings.longEvery} testId="pomo-long-every" onCommit={(v) => saveSettings({ longEvery: v })} />
          <label className="pomo-check"><input type="checkbox" data-testid="pomo-autostart" checked={data.settings.autoStart} onChange={(e) => saveSettings({ autoStart: e.target.checked })} /> Start the next phase automatically</label>
          <label className="pomo-check"><input type="checkbox" data-testid="pomo-sound" checked={data.settings.sound} onChange={(e) => saveSettings({ sound: e.target.checked })} /> Play a chime when a phase ends</label>
          <div className="pomo-note">Changes apply to phases that have not started yet. A phase that is running keeps its length.</div>
        </div>
      )}
    </div>
  );
}
