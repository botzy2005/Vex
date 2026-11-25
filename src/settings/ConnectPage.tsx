import { useBackend } from '../backend/hooks';
import type { Settings, SettingsPatch } from '../shared/types';
import { CommitText, Row, Toggle } from './controls';

export function ConnectPage({ s, update }: { s: Settings; update(p: SettingsPatch): Promise<void> }) {
  const c = s.connect;
  const backend = useBackend();
  const set = (p: Partial<Settings['connect']>) => update({ connect: p });
  return (
    <section>
      <h2>Phone (VEX Connect)</h2>
      <p className="section-note" data-testid="cx-privacy">
        VEX Connect pairs your phone with this PC over your own Wi-Fi. While it is off, VEX does not listen on the network and does not announce itself. When it is on, only devices on your private network can reach it, and only phones you paired and confirmed on both screens can connect. Nothing is sent to the internet.
      </p>
      <Row label="Turn on VEX Connect" hint="Windows may ask once to allow VEX on private networks. Allow private networks only.">
        <Toggle label="Turn on VEX Connect" testId="cx-set-enabled" checked={c.enabled} onChange={(v) => set({ enabled: v })} />
      </Row>
      <Row label="Name shown on your phone" hint="Leave empty to use this computer's name.">
        <CommitText label="Name shown on your phone" testId="cx-set-name" width={220} value={c.deviceName} placeholder="This computer's name" onCommit={(v) => set({ deviceName: v })} />
      </Row>
      <Row label="Port" hint="Only change this if another program uses it. Changing it restarts VEX Connect and phones reconnect by themselves.">
        <CommitText label="Port" testId="cx-set-port" width={90} value={String(c.port)} onCommit={(v) => { const n = Number(v); if (Number.isInteger(n)) void set({ port: n }); }} />
      </Row>
      <Row label="Pair and manage phones" hint="Show the pairing code and the list of paired phones.">
        <button className="reset" style={{ margin: 0 }} data-testid="cx-set-open" onClick={() => void backend.connectOpen()}>Open VEX Connect…</button>
      </Row>
    </section>
  );
}
