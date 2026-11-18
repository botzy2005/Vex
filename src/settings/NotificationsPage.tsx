import { useBackend } from '../backend/hooks';
import { AppIcon } from '../notifications/NotificationsPanel';
import { accessMessage } from '../notifications/group';
import { useNotifications } from '../notifications/useNotifications';
import type { Settings, SettingsPatch } from '../shared/types';
import { Row, Select, Toggle } from './controls';

const LIMITS = [25, 50, 100, 250, 500].map((n) => ({ value: n, label: `${n} notifications` }));

export function NotificationsPage({ s, update }: { s: Settings; update(p: SettingsPatch): Promise<void> }) {
  const n = s.notifications;
  const backend = useBackend();
  const { view } = useNotifications(false);
  const set = (p: Partial<Settings['notifications']>) => update({ notifications: p });
  const status = accessMessage(view.access, n.enabled);
  const limits = LIMITS.some((o) => o.value === n.historyLimit) ? LIMITS : [...LIMITS, { value: n.historyLimit, label: `${n.historyLimit} notifications` }];

  const toggleApp = (id: string, allowed: boolean) =>
    set({ blockedApps: allowed ? n.blockedApps.filter((a) => a !== id) : [...n.blockedApps, id] });

  return (
    <section>
      <h2>Notifications</h2>
      <p className="section-note" data-testid="notif-privacy">
        Notifications are read from Windows and stay on this PC, in memory only. They are never sent to an AI provider and are forgotten when VEX closes.
      </p>
      <Row label="Show Windows notifications in the island" hint="Windows asks for your permission the first time. Turning this off stops listening and clears what VEX holds.">
        <Toggle label="Show Windows notifications in the island" testId="notif-enabled" checked={n.enabled} onChange={(v) => set({ enabled: v })} />
      </Row>
      {n.enabled && (
        <div className="status-line" data-testid="notif-access" data-state={view.access.state}>
          {status ?? 'Windows has given VEX permission. New notifications will appear in the island.'}
        </div>
      )}
      <Row label="Show previews" hint="When off, only the app name is shown and the notification text is never kept.">
        <Toggle label="Show previews" testId="notif-previews" checked={n.showPreviews} onChange={(v) => set({ showPreviews: v })} />
      </Row>
      <Row label="Group by app" hint="Stack notifications from the same app together.">
        <Toggle label="Group by app" testId="notif-grouping" checked={n.grouping} onChange={(v) => set({ grouping: v })} />
      </Row>
      <Row label="History size" hint="The oldest notifications are dropped beyond this.">
        <Select<number> label="History size" testId="notif-limit" value={n.historyLimit} options={limits} onChange={(v) => set({ historyLimit: v })} />
      </Row>
      <Row label="History" hint={`${view.entries.length} kept right now.`}>
        <button className="reset" style={{ margin: 0 }} data-testid="notif-clear-history" disabled={view.entries.length === 0} onClick={() => void backend.notificationClearAll()}>Clear notification history</button>
      </Row>
      <h3>Apps</h3>
      {view.apps.length === 0 ? (
        <p className="section-note" data-testid="notif-apps-empty">Apps appear here after they send a notification. Switch one off to ignore it.</p>
      ) : (
        <div data-testid="notif-apps">
          {view.apps.map((a) => (
            <div className="row" key={a.id} data-testid="notif-app-row" data-app={a.id}>
              <div className="row-text" style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <AppIcon icon={a.icon} name={a.name} size={20} />
                <div className="row-label">{a.name}</div>
              </div>
              <div className="row-control">
                <Toggle label={`Show notifications from ${a.name}`} testId="notif-app-toggle" checked={!a.blocked} onChange={(v) => toggleApp(a.id, v)} />
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
