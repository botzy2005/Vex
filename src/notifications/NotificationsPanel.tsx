import { useEffect, useState } from 'react';
import { useBackend } from '../backend/hooks';
import { CloseIcon } from '../island/icons';
import type { NotificationEntry, NotificationsView, Settings } from '../shared/types';
import { accessMessage, groupEntries, relativeTime } from './group';

export function AppIcon({ icon, name, size = 22 }: { icon: string | null; name: string; size?: number }) {
  const [broken, setBroken] = useState(false);
  if (icon && !broken) {
    return <img className="n-icon" src={icon} alt="" width={size} height={size} onError={() => setBroken(true)} />;
  }
  // Missing or unreadable icon: a letter tile, never a broken image.
  return (
    <span className="n-icon n-letter" style={{ width: size, height: size, fontSize: size * 0.5 }} aria-hidden>
      {(name.trim()[0] ?? '?').toUpperCase()}
    </span>
  );
}

function useNow(everyMs = 30_000) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), everyMs);
    return () => window.clearInterval(t);
  }, [everyMs]);
  return now;
}

export function NotificationsPanel({ view, settings, onOpenSettings }: { view: NotificationsView; settings: Settings['notifications']; onOpenSettings(): void }) {
  const backend = useBackend();
  const now = useNow();
  const [note, setNote] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const groups = groupEntries(view.entries, settings.grouping, view.apps);
  const problem = accessMessage(view.access, settings.enabled);

  const open = async (e: NotificationEntry) => {
    setNote(null);
    try {
      await backend.notificationOpen(e.id);
    } catch (err) {
      setNote(String(err instanceof Error ? err.message : err));
      void backend.notificationDismiss(e.id);
    }
  };

  const toggle = (key: string) =>
    setCollapsed((c) => {
      const n = new Set(c);
      if (!n.delete(key)) n.add(key);
      return n;
    });

  return (
    <div className="notif-panel" data-testid="notifications-panel">
      <div className="notif-head">
        <span className="notif-count">{view.entries.length === 0 ? 'No notifications' : `${view.entries.length} notification${view.entries.length === 1 ? '' : 's'}`}</span>
        <span className="grow" />
        <button className="pill-btn" data-testid="notif-mark-read" disabled={view.unread === 0} onClick={() => void backend.notificationDismissAll()}>Mark all read</button>
        <button className="pill-btn" data-testid="notif-clear-all" disabled={view.entries.length === 0} onClick={() => void backend.notificationClearAll()}>Clear all</button>
      </div>
      {problem && (
        <div className="notif-empty" data-testid="notif-status">
          <p>{problem}</p>
          <button className="pill-btn" onClick={onOpenSettings}>Open settings</button>
        </div>
      )}
      {note && <div className="notif-note" role="status" data-testid="notif-note">{note}</div>}
      {!problem && view.entries.length === 0 && <div className="notif-empty" data-testid="notif-empty"><p>Nothing yet. New Windows notifications will appear here.</p></div>}
      <div className="notif-list" data-testid="notif-list">
        {groups.map((g) => {
          const multi = settings.grouping && g.items.length > 1;
          const shut = multi && collapsed.has(g.key);
          return (
            <section key={g.key} className="notif-group" data-testid="notif-group" data-app={g.appId} data-count={g.items.length}>
              {settings.grouping && (
                <header className="notif-group-head">
                  <AppIcon icon={g.icon} name={g.appName} size={18} />
                  <span className="notif-app">{g.appName}</span>
                  {g.unread > 0 && <span className="notif-dot" aria-label={`${g.unread} unread`} />}
                  {multi && (
                    <button className="notif-count-btn" aria-expanded={!shut} aria-label={`${shut ? 'Show' : 'Hide'} ${g.items.length} notifications from ${g.appName}`} data-testid="notif-group-toggle" onClick={() => toggle(g.key)}>
                      {g.items.length}
                    </button>
                  )}
                </header>
              )}
              {!shut &&
                g.items.map((e) => (
                  <div key={e.id} className="notif-item" data-testid="notif-item" data-unread={e.unread}>
                    {!settings.grouping && <AppIcon icon={g.icon} name={e.appName} />}
                    <button className="notif-main" onClick={() => void open(e)} title={`Open ${e.appName}`}>
                      {!settings.grouping && <span className="notif-app-line">{e.appName}</span>}
                      <span className="notif-title">{e.title || (e.body ? '' : 'New notification')}</span>
                      {e.body && <span className="notif-body">{e.body}</span>}
                    </button>
                    <span className="notif-time">{relativeTime(e.tsMs, now)}</span>
                    <button className="icon-btn small" aria-label={`Remove notification from ${e.appName}`} title="Remove from history" data-testid="notif-clear" onClick={() => void backend.notificationClear(e.id)}>
                      <CloseIcon width={12} height={12} />
                    </button>
                  </div>
                ))}
            </section>
          );
        })}
      </div>
    </div>
  );
}

/** The collapsed island's notification preview. */
export function NotificationPreview({ entry, icon, onDismiss }: { entry: NotificationEntry; icon: string | null; onDismiss(): void }) {
  return (
    <div className="v compact notif-compact" data-testid="view-notification">
      <AppIcon icon={icon} name={entry.appName} size={26} />
      <div className="grow notif-text">
        <span className="notif-app-line">{entry.appName}</span>
        <span className="compact-title">{entry.title || entry.body || 'New notification'}</span>
      </div>
      <button
        className="icon-btn small"
        aria-label="Dismiss notification"
        data-testid="notif-preview-dismiss"
        onClick={(ev) => {
          ev.stopPropagation(); // do not also expand the island
          onDismiss();
        }}
      >
        <CloseIcon width={12} height={12} />
      </button>
    </div>
  );
}
