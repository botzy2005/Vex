import { useCallback, useEffect, useState } from 'react';
import { useBackend, useSettings, useSubscription } from '../backend/hooks';
import type { ConnectDevice, ConnectPairing, ConnectStatus } from '../shared/types';

function seen(ms: number | null): string {
  if (!ms) return 'Never connected';
  const d = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (d < 60) return 'Seen just now';
  if (d < 3600) return `Seen ${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `Seen ${Math.floor(d / 3600)} h ago`;
  return `Seen ${Math.floor(d / 86400)} d ago`;
}

function Countdown({ until }: { until: number }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const s = Math.max(0, Math.ceil((until - now) / 1000));
  return (
    <span data-testid="cx-countdown">
      Expires in {Math.floor(s / 60)}:{String(s % 60).padStart(2, '0')}
    </span>
  );
}

function PairingCard({ p, run }: { p: ConnectPairing; run: (f: () => Promise<void>) => void }) {
  const backend = useBackend();
  if (p.state === 'waiting') {
    return (
      <div className="cx-card" data-testid="cx-pairing" data-state="waiting">
        <h2>Scan with VEX Connect</h2>
        <p className="cx-dim">Open VEX Connect on your phone, choose Pair a PC, and point the camera at this code. Both devices must be on the same Wi-Fi.</p>
        <div className="cx-qr" data-testid="cx-qr" dangerouslySetInnerHTML={{ __html: p.qrSvg }} />
        <p className="cx-dim cx-center"><Countdown until={p.expiresMs} /></p>
        <details className="cx-details">
          <summary>Can't scan? Use the code instead</summary>
          <textarea readOnly className="cx-code" data-testid="cx-codetext" value={p.codeText} onFocus={(e) => e.currentTarget.select()} />
          <p className="cx-dim">Paste this into "Enter code" on the phone. It works once and stops working when it expires.</p>
        </details>
        <div className="cx-actions"><button className="cx-btn" data-testid="cx-cancel" onClick={() => run(() => backend.connectCancelPairing())}>Cancel</button></div>
      </div>
    );
  }
  if (p.state === 'confirm' || p.state === 'waitingPeer') {
    return (
      <div className="cx-card" data-testid="cx-pairing" data-state={p.state}>
        <h2>Pair with {p.peerName}?</h2>
        <p className="cx-dim">Check that your phone shows the same code. If it does not, someone else may be trying to connect: choose No.</p>
        <div className="cx-sas" data-testid="cx-sas" aria-label={`Pairing code ${p.sas}`}>{p.sas}</div>
        {p.state === 'confirm' ? (
          <div className="cx-actions">
            <button className="cx-btn" data-testid="cx-no" onClick={() => run(() => backend.connectConfirmPairing(false))}>No, they differ</button>
            <button className="cx-btn primary" data-testid="cx-yes" onClick={() => run(() => backend.connectConfirmPairing(true))}>Yes, they match</button>
          </div>
        ) : (
          <>
            <p className="cx-dim cx-center" data-testid="cx-waiting">Waiting for you to confirm on the phone…</p>
            <div className="cx-actions"><button className="cx-btn" data-testid="cx-cancel" onClick={() => run(() => backend.connectCancelPairing())}>Cancel</button></div>
          </>
        )}
      </div>
    );
  }
  const good = p.state === 'paired';
  return (
    <div className="cx-card" data-testid="cx-pairing" data-state={p.state}>
      <h2>{good ? `Paired with ${p.peerName}` : p.state === 'expired' ? 'Code expired' : p.state === 'declined' ? 'Not paired' : 'Pairing stopped'}</h2>
      {p.message && <p className="cx-dim" data-testid="cx-pairing-message">{p.message}</p>}
      {good && <p className="cx-dim">This phone can now connect to this PC. You can remove it at any time from the list below.</p>}
      <div className="cx-actions">
        <button className="cx-btn" data-testid="cx-done" onClick={() => run(() => backend.connectCancelPairing())}>{good ? 'Done' : 'Close'}</button>
        {!good && <button className="cx-btn primary" data-testid="cx-retry" onClick={() => run(() => backend.connectBeginPairing())}>Try again</button>}
      </div>
    </div>
  );
}

function DeviceRow({ d, run }: { d: ConnectDevice; run: (f: () => Promise<void>) => void }) {
  const backend = useBackend();
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(d.name);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const save = () => {
    setEditing(false);
    if (name.trim() && name.trim() !== d.name) run(() => backend.connectRenameDevice(d.id, name));
    else setName(d.name);
  };
  return (
    <li className="cx-device" data-testid="cx-device" data-id={d.id} data-connected={d.connected}>
      <div className="cx-device-main">
        {editing ? (
          <input className="cx-input" data-testid="cx-rename-input" autoFocus maxLength={40} value={name} onChange={(e) => setName(e.target.value)} onBlur={save} onKeyDown={(e) => { if (e.key === 'Enter') save(); if (e.key === 'Escape') { setEditing(false); setName(d.name); } }} />
        ) : (
          <div className="cx-device-name" data-testid="cx-device-name">{d.name}</div>
        )}
        <div className="cx-dim cx-small">
          <span className="cx-dot" data-on={d.connected} aria-hidden="true" />
          <span data-testid="cx-device-status">{d.connected ? 'Connected' : seen(d.lastSeenMs)}</span> · <span title="Compare with the fingerprint shown on the phone">{d.fingerprint}</span>
        </div>
      </div>
      <div className="cx-device-actions">
        {confirmRemove ? (
          <>
            <span className="cx-small cx-dim">Remove this phone?</span>
            <button className="cx-btn danger" data-testid="cx-remove-yes" onClick={() => { setConfirmRemove(false); run(() => backend.connectRevokeDevice(d.id)); }}>Remove</button>
            <button className="cx-btn" data-testid="cx-remove-no" onClick={() => setConfirmRemove(false)}>Keep</button>
          </>
        ) : (
          <>
            <button className="cx-btn" data-testid="cx-rename" onClick={() => setEditing(true)}>Rename</button>
            {d.connected && <button className="cx-btn" data-testid="cx-disconnect" onClick={() => run(() => backend.connectDisconnectDevice(d.id))}>Disconnect</button>}
            <button className="cx-btn danger" data-testid="cx-remove" onClick={() => setConfirmRemove(true)}>Remove</button>
          </>
        )}
      </div>
    </li>
  );
}

export function ConnectApp() {
  const backend = useBackend();
  const { settings, update } = useSettings();
  const [status, setStatus] = useState<ConnectStatus | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const refresh = useCallback(() => {
    backend.connectStatus().then(setStatus).catch((e) => setErr(String(e instanceof Error ? e.message : e)));
  }, [backend]);
  useEffect(refresh, [refresh, settings?.connect.enabled, settings?.connect.port, settings?.connect.deviceName]);
  useSubscription((cb) => backend.onConnectChanged(() => cb(undefined)), refresh);

  const run = (f: () => Promise<void>) => {
    setErr(null);
    f().then(refresh, (e) => {
      setErr(String(e instanceof Error ? e.message : e));
      refresh();
    });
  };

  if (!settings || !status) return <div className="cx" />;
  const on = settings.connect.enabled;
  return (
    <div className="cx" data-testid="cx-root">
      <header className="cx-head">
        <div>
          <h1>VEX Connect</h1>
          <div className="cx-dim cx-small" data-testid="cx-name">This PC appears as “{status.name}”</div>
        </div>
        <button role="switch" aria-checked={on} aria-label="Turn on VEX Connect" className="cx-toggle" data-on={on} data-testid="cx-enabled" onClick={() => void update({ connect: { enabled: !on } })}>
          <span />
        </button>
      </header>

      {err && <div className="cx-notice" role="alert" data-testid="cx-error">{err}</div>}

      {!on ? (
        <div className="cx-card" data-testid="cx-off">
          <h2>VEX Connect is off</h2>
          <p className="cx-dim">While it is off, VEX does not listen on the network and does not announce itself. Turn it on to pair your phone. Nothing leaves this PC, and phones can only connect on your own Wi-Fi.</p>
        </div>
      ) : (
        <>
          {status.error && <div className="cx-notice" role="alert" data-testid="cx-status-error">{status.error}</div>}
          <p className="cx-dim cx-small" data-testid="cx-listening">
            {status.listening ? `Listening on your network (port ${status.port})${status.addresses.length ? ` · ${status.addresses.join(', ')}` : ''}` : 'Not listening'}
          </p>
          {status.pairing ? (
            <PairingCard p={status.pairing} run={run} />
          ) : (
            <div className="cx-card">
              <h2>Pair a phone</h2>
              <p className="cx-dim">A pairing code works once, for two minutes. You will confirm on both devices before anything is trusted.</p>
              <div className="cx-actions"><button className="cx-btn primary" data-testid="cx-pair" disabled={!status.listening} onClick={() => run(() => backend.connectBeginPairing())}>Pair a new phone</button></div>
            </div>
          )}
          <h2 className="cx-h2">Paired phones</h2>
          {status.devices.length === 0 ? (
            <p className="cx-dim" data-testid="cx-empty">No phones are paired yet.</p>
          ) : (
            <ul className="cx-list" data-testid="cx-devices">
              {status.devices.map((d) => <DeviceRow key={d.id} d={d} run={run} />)}
            </ul>
          )}
        </>
      )}
    </div>
  );
}
