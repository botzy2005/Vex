import { useBackend } from '../backend/hooks';
import { GridIcon, TimerIcon } from './icons';
import { entryOf } from './registry';
import { formatRemaining, remainingMs } from './timeFormat';
import { useHubLayout, useHubStatuses, useTimers } from './useHub';

const MAX_TIMERS_SHOWN = 4;

/**
 * The island's side of the Hub: one-click access to pinned widgets, the Hub itself, any line a widget
 * asked the island to show, and the shared timers. It never loads widget code.
 */
export function WidgetsPanel() {
  const backend = useBackend();
  const { catalog, layout } = useHubLayout();
  const statuses = useHubStatuses();
  const { timers, now } = useTimers(true);

  const open = (kind?: string) => void backend.hubOpen(kind).catch(() => {});
  const pinned = layout?.widgets.filter((w) => w.pinned) ?? [];
  const title = (kind: string) => catalog.find((d) => d.id === kind)?.title ?? kind;
  const live = [...timers].filter((t) => t.state !== 'idle').sort((a, b) => remainingMs(a, now) - remainingMs(b, now));
  const ordered = [...statuses].sort((a, b) => b.priority - a.priority);

  return (
    <div className="wt" data-testid="widgets-tab">
      <div className="wt-head">
        <span className="wt-title">Widgets</span>
        <span className="grow" />
        <button className="pill-btn" data-testid="open-hub" onClick={() => open()}>
          <GridIcon width={13} height={13} /> Open Hub
        </button>
      </div>

      {ordered.length > 0 && (
        <div className="wt-statuses" data-testid="widget-statuses">
          {ordered.map((s) => (
            <button key={s.kind} className="wt-status" data-testid="widget-status" data-priority={s.priority} onClick={() => open(s.kind)}>
              <span className="wt-status-label">{s.label}</span>
              {s.detail && <span className="wt-status-detail">{s.detail}</span>}
            </button>
          ))}
        </div>
      )}

      {pinned.length === 0 ? (
        <div className="wt-empty" data-testid="widgets-empty">
          <strong>No pinned widgets</strong>
          Pin a widget in the Hub to reach it from here.
        </div>
      ) : (
        <div className="wt-grid" data-testid="pinned-list">
          {pinned.map((p) => {
            const Icon = entryOf(p.kind)?.icon ?? GridIcon;
            return (
              <button key={p.kind} className="wt-tile" data-testid="pinned-widget" data-kind={p.kind} onClick={() => open(p.kind)} title={`Open ${title(p.kind)} in the Hub`}>
                <Icon width={20} height={20} />
                <span>{title(p.kind)}</span>
              </button>
            );
          })}
        </div>
      )}

      {live.length > 0 && (
        <div className="wt-timers" data-testid="island-timers">
          {live.slice(0, MAX_TIMERS_SHOWN).map((t) => (
            <button key={t.id} className="wt-timer" data-testid="island-timer" data-state={t.state} onClick={() => open(t.owner)} title="Open in the Hub">
              <TimerIcon width={14} height={14} />
              <span className="wt-timer-label">{t.label || 'Timer'}</span>
              <span className="wt-timer-time">{t.state === 'finished' ? 'Done' : t.state === 'paused' ? `Paused ${formatRemaining(remainingMs(t, now))}` : formatRemaining(remainingMs(t, now))}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
