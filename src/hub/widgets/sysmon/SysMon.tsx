import { useEffect, useRef, useState } from 'react';
import { useBackend } from '../../../backend/hooks';
import type { SysSnapshot } from '../../../shared/types';
import type { WidgetProps } from '../../registry';
import { useWidgetData } from '../../useWidgetData';
import { barWidth, batteryLabel, DEFAULT_DOC, formatBytes, formatRate, INTERVALS, level, normalize, type SysDoc } from './logic';
import './sysmon.css';

/** Polls only while `active` (the Hub is visible); one request at a time; stops on unmount. */
function useSnapshot(active: boolean, intervalMs: number) {
  const backend = useBackend();
  const [snap, setSnap] = useState<SysSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!active) return;
    let dead = false;
    let t: number;
    void backend.sysmonReset().catch(() => {});
    const tick = async () => {
      try {
        const s = await backend.sysmonSample();
        if (dead) return;
        setSnap(s);
        setError(s.available ? null : s.error ?? 'Not available');
      } catch (e) {
        if (!dead) setError(e instanceof Error ? e.message : String(e));
      }
      if (!dead) t = window.setTimeout(tick, intervalMs);
    };
    void tick();
    return () => {
      dead = true;
      window.clearTimeout(t);
    };
  }, [active, intervalMs, backend]);
  return { snap, error };
}

function Meter({ id, label, value, text, sub, low }: { id: string; label: string; value: number | null; text: string; sub?: string; low?: boolean }) {
  // for the battery a LOW number is the bad one
  const lv = low ? level(value === null ? null : 100 - value) : level(value);
  return (
    <div className="sm-meter" data-testid={`sm-${id}`} data-level={lv}>
      <div className="sm-line"><span>{label}</span><b data-testid={`sm-${id}-value`}>{text}</b></div>
      <div className="sm-bar" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value ?? undefined}>
        <i style={{ width: `${barWidth(value)}%` }} />
      </div>
      {sub && <small data-testid={`sm-${id}-sub`}>{sub}</small>}
    </div>
  );
}

export default function SysMon({ size, active }: WidgetProps) {
  const { value: doc, update, loaded } = useWidgetData<SysDoc>('sysmon', DEFAULT_DOC, normalize);
  const { snap, error } = useSnapshot(active && loaded, doc.intervalMs);
  const shown = useRef(false);
  if (snap) shown.current = true;

  return (
    <div className="sm" data-size={size} data-testid="widget-sysmon" data-view={error && !snap ? 'error' : snap ? 'ready' : 'loading'}>
      {error && (
        <div className="sm-problem" role="alert" data-testid="sm-error">
          {snap ? `Could not refresh: ${error}` : `System Monitor is not available: ${error}`}
        </div>
      )}
      {snap && snap.available && (
        <>
          <Meter id="cpu" label="CPU" value={snap.cpuPercent} text={snap.cpuPercent === null ? '…' : `${snap.cpuPercent}%`} />
          <Meter id="mem" label="Memory" value={snap.memPercent} text={`${snap.memPercent}%`} sub={`${formatBytes(snap.memUsedBytes)} of ${formatBytes(snap.memTotalBytes)}`} />
          <div className="sm-net" data-testid="sm-net">
            <span>Network</span>
            <span data-testid="sm-down">↓ {formatRate(snap.downBps)}</span>
            <span data-testid="sm-up">↑ {formatRate(snap.upBps)}</span>
          </div>
          {snap.battery ? (
            <Meter id="battery" label="Battery" value={snap.battery.percent} low text={`${snap.battery.percent}%`} sub={batteryLabel(snap.battery)} />
          ) : (
            <div className="sm-nobattery" data-testid="sm-nobattery">No battery detected</div>
          )}
        </>
      )}
      <div className="sm-foot">
        <span>Refresh</span>
        {INTERVALS.map((i) => (
          <button key={i} className="chip" data-testid={`sm-int-${i / 1000}`} aria-pressed={doc.intervalMs === i} onClick={() => update({ ...doc, intervalMs: i })}>
            {i / 1000} s
          </button>
        ))}
      </div>
    </div>
  );
}
