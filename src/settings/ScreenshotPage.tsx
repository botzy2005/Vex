import { useState } from 'react';
import { useBackend } from '../backend/hooks';
import type { CaptureMode, ImageFormat, Settings, SettingsPatch, ShotHistory } from '../shared/types';
import { HotkeyInput, Row, Segmented, Select, Toggle } from './controls';

const MODES: Array<{ value: CaptureMode; label: string }> = [
  { value: 'region', label: 'A selected area' },
  { value: 'window', label: 'The active window' },
  { value: 'screen', label: 'The full screen' },
];
const HISTORY: Array<{ value: ShotHistory; label: string }> = [
  { value: 'off', label: 'Off (latest only, in memory)' },
  { value: 'session', label: 'This session (last 10, in memory)' },
  { value: 'keep', label: 'Keep on this PC (last 30)' },
];
const DEFAULT_HOTKEY = 'Ctrl+Alt+S';

export function ScreenshotPage({ s, update }: { s: Settings; update(p: SettingsPatch): Promise<void> }) {
  const c = s.screenshot;
  const backend = useBackend();
  const [err, setErr] = useState<string | null>(null);
  const set = (p: Partial<Settings['screenshot']>) => update({ screenshot: p });
  const clash = !!c.hotkey && c.hotkey.toLowerCase() === s.island.hotkey.toLowerCase();

  const pick = async () => {
    setErr(null);
    try {
      const dir = await backend.screenshotPickFolder();
      if (dir) await set({ saveDir: dir });
    } catch (e) {
      setErr(String(e instanceof Error ? e.message : e));
    }
  };

  return (
    <section>
      <h2>Screenshot</h2>
      <p className="section-note" data-testid="shot-privacy">
        Screenshots stay on this PC. Nothing is captured in the background, only when you ask, and nothing is uploaded or shared unless you do that yourself. The VEX island is left out of the picture.
      </p>
      <Row label="Default format" hint="Used by Save. PNG is lossless; JPEG is smaller but loses detail, and transparent areas become white.">
        <Segmented<ImageFormat> label="Default format" testId="shot-format" value={c.format} onChange={(v) => set({ format: v })} options={[{ value: 'png', label: 'PNG' }, { value: 'jpeg', label: 'JPEG' }]} />
      </Row>
      <Row label="Save location" hint={c.saveDir ? c.saveDir : 'Default: the VEX Screenshots folder inside Pictures.'}>
        <span style={{ display: 'inline-flex', gap: 6 }}>
          <button className="reset" style={{ margin: 0 }} data-testid="shot-choose-dir" onClick={() => void pick()}>Choose…</button>
          <button className="reset" style={{ margin: 0 }} data-testid="shot-reset-dir" disabled={!c.saveDir} onClick={() => void set({ saveDir: '' })}>Use default</button>
        </span>
      </Row>
      {err && <div className="warning" role="alert" data-testid="shot-dir-error">{err}</div>}
      <Row label="Capture shortcut" hint="Needs Ctrl, Alt or Win. Turn it off if you only want to capture from the island.">
        <span style={{ display: 'inline-flex', gap: 6 }}>
          {c.hotkey ? <HotkeyInput testId="shot-hotkey" value={c.hotkey} onChange={(v) => set({ hotkey: v })} /> : <span className="ed-dim" data-testid="shot-hotkey-off">Off</span>}
          {c.hotkey
            ? <button className="reset" style={{ margin: 0 }} data-testid="shot-hotkey-clear" onClick={() => void set({ hotkey: '' })}>Turn off</button>
            : <button className="reset" style={{ margin: 0 }} data-testid="shot-hotkey-default" onClick={() => void set({ hotkey: DEFAULT_HOTKEY })}>Set {DEFAULT_HOTKEY}</button>}
        </span>
      </Row>
      {clash && <div className="warning" role="alert" data-testid="shot-hotkey-clash">This is the same shortcut as the island's, so only one of them will work. Pick a different one.</div>}
      <Row label="The shortcut captures">
        <Select<CaptureMode> label="The shortcut captures" testId="shot-hotkey-mode" value={c.hotkeyMode} options={MODES} onChange={(v) => set({ hotkeyMode: v })} />
      </Row>
      <Row label="Capture sound" hint="A short click when a screenshot is taken.">
        <Toggle label="Capture sound" testId="shot-sound" checked={c.sound} onChange={(v) => set({ sound: v })} />
      </Row>
      <Row label="Screenshot history" hint="Kept history is saved in VEX's data folder, not encrypted. Choosing another option deletes those files.">
        <Select<ShotHistory> label="Screenshot history" testId="shot-history" value={c.history} options={HISTORY} onChange={(v) => set({ history: v })} />
      </Row>
      <Row label="Clear history">
        <button className="reset" style={{ margin: 0 }} data-testid="shot-clear-history" onClick={() => void backend.screenshotClear()}>Clear screenshot history</button>
      </Row>
    </section>
  );
}
