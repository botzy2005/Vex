/**
 * TEST HARNESS — NOT PART OF THE SHIPPED APP.
 *
 * Browser stand-in for VEX Connect's native side. The real pairing, trust and session rules live in Rust
 * (`vex-link`, `vex_core::connect`, `src-tauri/src/connect`, unit- and loopback-tested there). This keeps just enough
 * behaviour for the UI tests, and lets a test play the phone: scan the code, answer the comparison, connect, disconnect.
 */
import type { ConnectDevice, ConnectPairing, ConnectStatus, Settings } from '../shared/types';
import type { Backend, Unsub } from './backend';

export interface ConnectHarness {
  /** The phone scans the QR code and the 6-digit code appears on the PC. */
  phoneScans(name: string): void;
  /** The phone user answers the 6-digit comparison. */
  phoneAnswers(accept: boolean): void;
  /** Let the code run out. */
  expireCode(): void;
  /** Pretend a paired phone connected or dropped. */
  setConnected(id: string, on: boolean): void;
  /** Make the next action fail with this message. */
  failNext(message: string | null): void;
  setError(message: string | null): void;
  readonly devices: ConnectDevice[];
  readonly calls: string[];
  reset(): void;
}

const SAS = '482 913';

export function createTestConnect(getSettings: () => Settings) {
  let devices: ConnectDevice[] = [];
  let pairing: ConnectPairing | null = null;
  let mine: boolean | null = null;
  let theirs: boolean | null = null;
  let fail: string | null = null;
  let error: string | null = null;
  let counter = 0;
  const calls: string[] = [];
  const subs = new Set<() => void>();
  const changed = () => subs.forEach((cb) => cb());
  const check = () => {
    if (fail) {
      const m = fail;
      fail = null;
      throw new Error(m);
    }
  };

  const settle = () => {
    if (!pairing || pairing.state === 'paired') return;
    if (mine === false || theirs === false) {
      pairing = { ...pairing, state: 'declined', sas: null, message: 'Pairing was declined.' };
    } else if (mine && theirs) {
      const name = pairing.peerName ?? 'Phone';
      const unique = devices.some((d) => d.name === name) ? `${name} (2)` : name;
      counter++;
      devices = [...devices, { id: `dev${counter}`, name: unique, fingerprint: `a1b2-c3d4-e5f6-${String(counter).padStart(4, '0')}`, addedMs: Date.now(), lastSeenMs: Date.now(), connected: false }];
      pairing = { ...pairing, state: 'paired', sas: null, peerName: unique };
    } else if (mine) {
      pairing = { ...pairing, state: 'waitingPeer' };
    }
  };

  const api: Pick<Backend, 'connectOpen' | 'connectStatus' | 'connectBeginPairing' | 'connectCancelPairing' | 'connectConfirmPairing' | 'connectRenameDevice' | 'connectDisconnectDevice' | 'connectRevokeDevice' | 'onConnectChanged'> = {
    async connectOpen() {
      calls.push('open');
    },
    async connectStatus(): Promise<ConnectStatus> {
      const c = getSettings().connect;
      return { enabled: c.enabled, listening: c.enabled && !error, port: c.port, name: c.deviceName || 'TEST-PC', error: c.enabled ? error : null, addresses: ['192.168.1.20'], pairing, devices };
    },
    async connectBeginPairing() {
      calls.push('begin');
      check();
      if (!getSettings().connect.enabled) throw new Error('Turn VEX Connect on first.');
      mine = theirs = null;
      pairing = {
        state: 'waiting',
        qrSvg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" data-testid="qr-svg"><rect width="10" height="10" fill="#fff"/><rect x="1" y="1" width="3" height="3"/></svg>',
        codeText: 'vexlink:TESTCODE',
        expiresMs: Date.now() + 120_000,
        sas: null,
        peerName: null,
        message: null,
      };
      changed();
    },
    async connectCancelPairing() {
      calls.push('cancel');
      pairing = null;
      changed();
    },
    async connectConfirmPairing(accept) {
      calls.push(`confirm:${accept}`);
      check();
      if (!pairing || pairing.state !== 'confirm') throw new Error('There is nothing to confirm.');
      mine = accept;
      settle();
      changed();
    },
    async connectRenameDevice(id, name) {
      calls.push(`rename:${id}:${name}`);
      check();
      const n = name.trim();
      if (!n) throw new Error('That name or device is not valid.');
      if (!devices.some((d) => d.id === id)) throw new Error('That name or device is not valid.');
      devices = devices.map((d) => (d.id === id ? { ...d, name: devices.some((o) => o.id !== id && o.name.toLowerCase() === n.toLowerCase()) ? `${n} (2)` : n } : d));
      changed();
    },
    async connectDisconnectDevice(id) {
      calls.push(`disconnect:${id}`);
      devices = devices.map((d) => (d.id === id ? { ...d, connected: false } : d));
      changed();
    },
    async connectRevokeDevice(id) {
      calls.push(`revoke:${id}`);
      check();
      if (!devices.some((d) => d.id === id)) throw new Error('That device is not paired.');
      devices = devices.filter((d) => d.id !== id);
      changed();
    },
    async onConnectChanged(cb): Promise<Unsub> {
      subs.add(cb);
      return () => void subs.delete(cb);
    },
  };

  const harness: ConnectHarness = {
    phoneScans(name) {
      if (!pairing || pairing.state !== 'waiting') throw new Error('no code is showing');
      pairing = { ...pairing, state: 'confirm', qrSvg: '', codeText: '', sas: SAS, peerName: name };
      changed();
    },
    phoneAnswers(accept) {
      theirs = accept;
      settle();
      changed();
    },
    expireCode() {
      if (pairing?.state === 'waiting') pairing = { ...pairing, state: 'expired', qrSvg: '', codeText: '', message: 'This code has expired.' };
      changed();
    },
    setConnected(id, on) {
      devices = devices.map((d) => (d.id === id ? { ...d, connected: on, lastSeenMs: Date.now() } : d));
      changed();
    },
    failNext(m) {
      fail = m;
    },
    setError(m) {
      error = m;
      changed();
    },
    get devices() {
      return devices;
    },
    get calls() {
      return calls;
    },
    reset() {
      devices = [];
      pairing = null;
      mine = theirs = fail = error = null;
      calls.length = 0;
      changed();
    },
  };
  return { api, harness };
}
