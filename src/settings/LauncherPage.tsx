import { useCallback, useEffect, useState } from 'react';
import { useBackend } from '../backend/hooks';
import type { LauncherAiInfo, LauncherView, Settings, SettingsPatch, WebEngine } from '../shared/types';
import { HotkeyInput, Row, Select, Slider, Toggle } from './controls';

const DEFAULT_HOTKEY = 'Alt+Space';
const ENGINES: Array<{ value: WebEngine; label: string }> = [
  { value: 'duckduckgo', label: 'DuckDuckGo' },
  { value: 'google', label: 'Google' },
  { value: 'bing', label: 'Bing' },
];
const DEFAULT_FOLDERS = 'Desktop, Documents, Downloads, Pictures, Videos and Music';

function ago(secs: number | null): string {
  if (!secs) return 'not yet';
  const d = Math.max(0, Math.floor(Date.now() / 1000) - secs);
  if (d < 60) return 'just now';
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  return `${Math.floor(d / 86400)} d ago`;
}

export function LauncherPage({ s, update }: { s: Settings; update(p: SettingsPatch): Promise<void> }) {
  const l = s.launcher;
  const backend = useBackend();
  const [view, setView] = useState<LauncherView | null>(null);
  const [ai, setAi] = useState<LauncherAiInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const set = (p: Partial<Settings['launcher']>) => update({ launcher: p });

  const refresh = useCallback(() => {
    backend.launcherState().then(setView).catch((e) => setErr(String(e instanceof Error ? e.message : e)));
  }, [backend]);
  useEffect(() => {
    refresh();
    backend.launcherAiInfo().then(setAi).catch(() => setAi(null));
    const off = backend.onLauncherIndex(refresh);
    return () => void off.then((f) => f());
  }, [backend, refresh]);

  const clash = !!l.hotkey && [s.island.hotkey, s.screenshot.hotkey].some((h) => h && h.toLowerCase() === l.hotkey.toLowerCase());

  const addFolder = async () => {
    setErr(null);
    try {
      const dir = await backend.screenshotPickFolder();
      if (dir && !l.fileFolders.some((f) => f.toLowerCase() === dir.toLowerCase())) await set({ fileFolders: [...l.fileFolders, dir] });
    } catch (e) {
      setErr(String(e instanceof Error ? e.message : e));
    }
  };

  const unpin = async (id: string) => {
    setErr(null);
    try {
      await backend.launcherSetPin(id, false);
      refresh();
    } catch (e) {
      setErr(String(e instanceof Error ? e.message : e));
    }
  };

  const clear = async () => {
    await backend.launcherClearHistory();
    setNote('Recent launches and search history cleared.');
    refresh();
  };

  return (
    <section>
      <h2>Launcher</h2>
      <p className="section-note" data-testid="lx-privacy">
        The launcher searches this PC. Typed text only leaves it when you run a web search (it is sent to your search engine through your browser) or an AI command (it is sent to the AI provider you set up in Assistant). Nothing is read from your clipboard or screen, and recent launches and searches stay on this PC.
      </p>
      <Row label="Turn on the launcher" hint="A search window that opens from a shortcut.">
        <Toggle label="Turn on the launcher" testId="lx-enabled" checked={l.enabled} onChange={(v) => set({ enabled: v })} />
      </Row>
      <Row label="Shortcut" hint="Needs Ctrl, Alt or Win. Alt+Space is the default; if another app already uses it, VEX tells you here and you can pick another.">
        <span style={{ display: 'inline-flex', gap: 6, alignItems: 'center' }}>
          {l.hotkey ? <HotkeyInput testId="lx-hotkey" value={l.hotkey} onChange={(v) => set({ hotkey: v })} /> : <span className="ed-dim" data-testid="lx-hotkey-off">Off</span>}
          {l.hotkey
            ? <button className="reset" style={{ margin: 0 }} data-testid="lx-hotkey-clear" onClick={() => void set({ hotkey: '' })}>Turn off</button>
            : <button className="reset" style={{ margin: 0 }} data-testid="lx-hotkey-default" onClick={() => void set({ hotkey: DEFAULT_HOTKEY })}>Set {DEFAULT_HOTKEY}</button>}
        </span>
      </Row>
      {clash && <div className="warning" role="alert" data-testid="lx-hotkey-clash">This is the same shortcut as another VEX shortcut, so only one of them will work. Pick a different one.</div>}

      <h3 className="sub">What to search</h3>
      <Row label="Applications" hint="Installed desktop and Microsoft Store apps. The list is cached, not scanned as you type.">
        <Toggle label="Applications" testId="lx-apps" checked={l.apps} onChange={(v) => set({ apps: v })} />
      </Row>
      <Row label="Files and folders" hint="Names in the folders chosen below, from a cached index.">
        <Toggle label="Files and folders" testId="lx-files" checked={l.files} onChange={(v) => set({ files: v })} />
      </Row>
      <Row label="Quick commands" hint="calc, time, date, open, timer, note.">
        <Toggle label="Quick commands" testId="lx-commands" checked={l.commands} onChange={(v) => set({ commands: v })} />
      </Row>
      <Row label="Web search" hint="Adds a 'Search the web' row. Nothing is sent until you choose it, and it opens in your default browser.">
        <Toggle label="Web search" testId="lx-web" checked={l.web} onChange={(v) => set({ web: v })} />
      </Row>
      {l.web && (
        <Row label="Search engine">
          <Select<WebEngine> label="Search engine" testId="lx-engine" value={l.webEngine} options={ENGINES} onChange={(v) => set({ webEngine: v })} />
        </Row>
      )}
      <Row label="AI commands" hint="ask, explain, summarize, rewrite and translate. They run only when you press Enter on them, send only the text you typed, and use your Assistant settings and saved key.">
        <Toggle label="AI commands" testId="lx-ai" checked={l.ai} onChange={(v) => set({ ai: v })} />
      </Row>
      {l.ai && ai && (
        <div className="status-line" data-testid="lx-ai-status" data-ok={ai.ready}>
          {ai.ready ? `Will use ${ai.provider}, model ${ai.model}.` : ai.problem}
        </div>
      )}

      <h3 className="sub">Results</h3>
      <Row label="Maximum results" hint="Rows shown for a search.">
        <Slider label="Maximum results" testId="lx-max" value={l.maxResults} min={3} max={12} step={1} onChange={(v) => set({ maxResults: v })} format={(v) => `${v} rows`} />
      </Row>
      <Row label="Remember recent launches and searches" hint="Used to show recent apps and rank things you open often. Stays on this PC.">
        <Toggle label="Remember recent launches and searches" testId="lx-recents" checked={l.rememberRecents} onChange={(v) => set({ rememberRecents: v })} />
      </Row>
      <Row label="Clear history" hint={view ? `${view.recents.length} recent app${view.recents.length === 1 ? '' : 's'}, ${view.history.length} saved search${view.history.length === 1 ? '' : 'es'}. Pinned apps stay.` : undefined}>
        <button className="reset" style={{ margin: 0 }} data-testid="lx-clear" onClick={() => void clear()}>Clear history</button>
      </Row>
      {note && <div className="status-line" role="status" data-testid="lx-note">{note}</div>}

      <h3 className="sub">File search folders</h3>
      <p className="section-note">
        {l.fileFolders.length === 0 ? `Searching: ${DEFAULT_FOLDERS}.` : 'Searching only the folders below. Folders you cannot read are skipped.'}
      </p>
      <ul className="lx-folders" data-testid="lx-folders">
        {l.fileFolders.map((f) => (
          <li key={f}>
            <code>{f}</code>
            <button className="reset" style={{ margin: 0 }} aria-label={`Remove ${f}`} data-testid="lx-folder-remove" onClick={() => void set({ fileFolders: l.fileFolders.filter((x) => x !== f) })}>Remove</button>
          </li>
        ))}
      </ul>
      <div style={{ display: 'flex', gap: 6, margin: '6px 0 4px' }}>
        <button className="reset" style={{ margin: 0 }} data-testid="lx-folder-add" onClick={() => void addFolder()}>Add a folder…</button>
        <button className="reset" style={{ margin: 0 }} data-testid="lx-folder-default" disabled={l.fileFolders.length === 0} onClick={() => void set({ fileFolders: [] })}>Use the standard folders</button>
      </div>
      {view && (
        <div className="status-line" data-testid="lx-index-status">
          {view.appsCount} apps (checked {ago(view.appsUpdated)}) · {view.filesCount.toLocaleString()} files and folders (indexed {ago(view.filesUpdated)})
          {view.filesTruncated && ' · the index hit its size limit, so very large folders are only partly covered'}
          {view.filesUnreadable > 0 && ` · ${view.filesUnreadable} folder${view.filesUnreadable === 1 ? '' : 's'} could not be read and were skipped`}
        </div>
      )}
      <button className="reset" style={{ margin: '0 0 8px' }} data-testid="lx-refresh" disabled={view?.refreshing} onClick={() => void backend.launcherRefresh().then(() => setTimeout(refresh, 500))}>
        {view?.refreshing ? 'Refreshing…' : 'Refresh apps and files now'}
      </button>

      <h3 className="sub">Pinned apps</h3>
      {view && view.pins.length === 0 && <p className="section-note" data-testid="lx-no-pins">No pinned apps. In the launcher, select an app and press Ctrl+P, or click its star.</p>}
      <ul className="lx-folders" data-testid="lx-pins">
        {view?.pins.map((p) => (
          <li key={p.id}>
            <span>{p.name}</span>
            <button className="reset" style={{ margin: 0 }} aria-label={`Unpin ${p.name}`} data-testid="lx-unpin" onClick={() => void unpin(p.id)}>Remove</button>
          </li>
        ))}
      </ul>
      {err && <div className="warning" role="alert" data-testid="lx-error">{err}</div>}
    </section>
  );
}
