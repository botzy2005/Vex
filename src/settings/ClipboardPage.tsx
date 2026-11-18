import { useBackend } from '../backend/hooks';
import { formatBytes, statusLine } from '../clipboard/format';
import { useClipboard } from '../clipboard/useClipboard';
import type { Settings, SettingsPatch } from '../shared/types';
import { Row, Select, Toggle } from './controls';

const SIZES = [10, 25, 50, 100, 200].map((n) => ({ value: n, label: `${n} entries` }));

export function ClipboardPage({ s, update }: { s: Settings; update(p: SettingsPatch): Promise<void> }) {
  const c = s.clipboard;
  const backend = useBackend();
  const { status } = useClipboard(false);
  const set = (p: Partial<Settings['clipboard']>) => update({ clipboard: p });
  const line = statusLine(status);
  const sizes = SIZES.some((o) => o.value === c.maxItems) ? SIZES : [...SIZES, { value: c.maxItems, label: `${c.maxItems} entries` }];

  return (
    <section>
      <h2>Clipboard</h2>
      <p className="section-note" data-testid="clip-privacy">
        Clipboard history stays on this PC. It is never sent to an AI provider and the assistant cannot see it. Anything you copy while it is recording is kept here, so leave it off if you do not want that.
      </p>
      <Row label="Keep a clipboard history" hint="Records text and images you copy, so you can find and reuse them from the island.">
        <Toggle label="Keep a clipboard history" testId="clip-enabled" checked={c.enabled} onChange={(v) => set({ enabled: v })} />
      </Row>
      <div className="status-line" data-testid="clip-settings-status" data-state={line.tone}>
        {line.text}
        {status.imageBytes > 0 && ` · images use ${formatBytes(status.imageBytes)}`}
      </div>
      <Row label="Pause recording" hint="Keeps what is saved but does not record new copies. Anything copied while paused is never read.">
        <Toggle label="Pause recording" testId="clip-paused" checked={c.paused} onChange={(v) => set({ paused: v })} />
      </Row>
      <Row label="History size" hint="The oldest entries are dropped beyond this. Pinned entries are kept.">
        <Select<number> label="History size" testId="clip-max" value={c.maxItems} options={sizes} onChange={(v) => set({ maxItems: v })} />
      </Row>
      <Row label="Include images" hint="Screenshots and copied pictures, stored as lossless PNG. Large images can use a lot of memory, so turn this off if you do not need them.">
        <Toggle label="Include images" testId="clip-images" checked={c.images} onChange={(v) => set({ images: v })} />
      </Row>
      <Row label="Skip things that look private" hint="Passwords from password managers are always skipped. This also skips text that looks like an API key, a private key or a card number.">
        <Toggle label="Skip things that look private" testId="clip-sensitive" checked={c.skipSensitive} onChange={(v) => set({ skipSensitive: v })} />
      </Row>
      <Row label="Keep history after VEX restarts" hint="Off: the history lives in memory and is gone when VEX closes. On: it is saved in VEX's data folder, not encrypted. Turning this off deletes the saved files.">
        <Toggle label="Keep history after VEX restarts" testId="clip-persist" checked={c.persist} onChange={(v) => set({ persist: v })} />
      </Row>
      <Row label="Clear history" hint={`${status.count} saved, ${status.pinned} pinned.`}>
        <button className="reset" style={{ margin: 0 }} data-testid="clip-clear-all" disabled={status.count === 0} onClick={() => void backend.clipboardClear(false)}>Clear all, including pinned</button>
      </Row>
    </section>
  );
}
