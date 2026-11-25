import { useEffect, useState } from 'react';
import { AssistantFace } from '../assistant/AssistantFace';
import { useBackend, useResolvedTheme, useSettings } from '../backend/hooks';
import type { AiTestResult, AssistantState, MonitorInfo, Settings, SettingsPatch } from '../shared/types';
import { DeveloperPage } from './DeveloperPage';
import { ClipboardPage } from './ClipboardPage';
import { AppearancePage } from './AppearancePage';
import { ScreenshotPage } from './ScreenshotPage';
import { LauncherPage } from './LauncherPage';
import { ConnectPage } from './ConnectPage';
import { NotificationsPage } from './NotificationsPage';
import { CommitText, HotkeyInput, Row, Segmented, Select, Slider, Toggle } from './controls';

type Page = 'appearance' | 'island' | 'music' | 'assistant' | 'notifications' | 'clipboard' | 'screenshot' | 'launcher' | 'connect' | 'developer' | 'about';
const PAGES: Array<{ id: Page; label: string }> = [
  { id: 'appearance', label: 'Appearance' },
  { id: 'island', label: 'Island' },
  { id: 'music', label: 'Music & lyrics' },
  { id: 'assistant', label: 'Assistant' },
  { id: 'notifications', label: 'Notifications' },
  { id: 'clipboard', label: 'Clipboard' },
  { id: 'screenshot', label: 'Screenshot' },
  { id: 'launcher', label: 'Launcher' },
  { id: 'connect', label: 'Phone' },
  { id: 'developer', label: 'Developer' },
  { id: 'about', label: 'About' },
];

const KEEP_ALIVE_OPTIONS = [
  { value: '5m', label: '5 minutes' },
  { value: '30m', label: '30 minutes' },
  { value: '2h', label: '2 hours' },
  { value: '-1', label: 'Until Ollama quits' },
  { value: '0', label: 'Unload right away' },
];
const CTX_OPTIONS = [2048, 4096, 8192, 16384].map((n) => ({ value: n, label: `${n.toLocaleString()} tokens` }));
/** Keeps a value set elsewhere (a hand-edited file) visible in a dropdown instead of silently hiding it. */
function keepCurrent<T extends string | number>(opts: Array<{ value: T; label: string }>, current: T) {
  return opts.some((o) => o.value === current) ? opts : [...opts, { value: current, label: String(current) }];
}

type Update = (p: SettingsPatch) => Promise<void>;

export function SettingsApp() {
  const { settings, update, warnings } = useSettings();
  const theme = useResolvedTheme(settings?.appearance.theme);
  const [page, setPage] = useState<Page>('appearance');

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  if (!settings) return null;

  return (
    <div className="settings" data-testid="settings">
      <aside>
        <h1>VEX</h1>
        <nav>
          {PAGES.map((p) => (
            <button key={p.id} data-active={page === p.id} data-testid={`page-${p.id}`} onClick={() => setPage(p.id)}>{p.label}</button>
          ))}
        </nav>
      </aside>
      <main>
        {warnings.map((w) => <div key={w} className="warning" role="alert" data-testid="warning">{w}</div>)}
        {page === 'appearance' && <AppearancePage s={settings} update={update} />}
        {page === 'island' && <IslandPage s={settings} update={update} />}
        {page === 'music' && <MusicPage s={settings} update={update} />}
        {page === 'assistant' && <AssistantPage s={settings} update={update} />}
        {page === 'clipboard' && <ClipboardPage s={settings} update={update} />}
        {page === 'screenshot' && <ScreenshotPage s={settings} update={update} />}
        {page === 'launcher' && <LauncherPage s={settings} update={update} />}
        {page === 'connect' && <ConnectPage s={settings} update={update} />}
        {page === 'notifications' && <NotificationsPage s={settings} update={update} />}
        {page === 'developer' && <DeveloperPage s={settings} />}
        {page === 'about' && <AboutPage />}
      </main>
    </div>
  );
}

function ResetButton({ onClick, testId }: { onClick(): void; testId?: string }) {
  return <button className="reset" onClick={onClick} data-testid={testId}>Reset section</button>;
}

function IslandPage({ s, update }: { s: Settings; update: Update }) {
  const i = s.island;
  const backend = useBackend();
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  useEffect(() => { backend.listMonitors().then(setMonitors).catch(() => setMonitors([])); }, [backend]);
  const set = (p: Partial<Settings['island']>) => update({ island: p });
  return (
    <section>
      <h2>Island</h2>
      <h3 className="sub" data-testid="group-position">Position</h3>
      <Row label="Display" hint="Which monitor the island appears on.">
        <Select<number> label="Display" testId="monitor" value={i.monitorIndex} onChange={(v) => set({ monitorIndex: v })}
          options={[{ value: -1, label: 'Primary display' }, ...monitors.map((m) => ({ value: m.index, label: `${m.name} (${m.width}×${m.height} @ ${Math.round(m.scale * 100)}%)` }))]} />
      </Row>
      <Row label="Position" hint="The island moves along the top edge only. Hold Alt and drag it, or use the handle that appears at its bottom edge when you hover it. Left or right of centre is shown here.">
        <div className="inline-row">
          <Slider label="Horizontal position" testId="offset" min={-2000} max={2000} step={5} value={Math.max(-2000, Math.min(2000, i.offsetX))} format={(v) => (v === 0 ? 'Centre' : v < 0 ? `${-v}px left` : `${v}px right`)} onChange={(v) => set({ offsetX: v })} />
          <button className="btn" data-testid="reset-position" disabled={i.offsetX === 0} onClick={() => set({ offsetX: 0 })}>Reset position</button>
        </div>
      </Row>
      <Row label="Gap from top edge"><Slider label="Gap from top edge" testId="margin" min={0} max={60} step={1} value={i.marginTop} format={(v) => `${v}px`} onChange={(v) => set({ marginTop: v })} /></Row>
      <h3 className="sub" data-testid="group-visibility">Visibility</h3>
      <Row label="Visibility" hint="Always visible keeps the island on screen. Auto-hide slides it away when the pointer leaves; move the pointer to the top edge of the screen, press the shortcut, or use the tray's Show VEX to bring it back. Notifications and timers never bring it back.">
        <Segmented<'always' | 'auto'> label="Visibility" testId="visibility" value={i.visibility} onChange={(v) => set({ visibility: v })}
          options={[{ value: 'always', label: 'Always visible' }, { value: 'auto', label: 'Auto-hide' }]} />
      </Row>
      {i.visibility === 'auto' && (
        <Row label="Hide delay" hint="How long the island waits, once nothing is using it, before it hides."><Slider label="Hide delay" testId="autohide-delay" min={200} max={5000} step={100} value={i.autoHideDelayMs} format={(v) => `${v} ms`} onChange={(v) => set({ autoHideDelayMs: v })} /></Row>
      )}
      <h3 className="sub" data-testid="group-behaviour">Behaviour</h3>
      <Row label="Expand on hover" hint="When off, click the island to expand it."><Toggle label="Expand on hover" testId="hover" checked={i.hoverToExpand} onChange={(v) => set({ hoverToExpand: v })} /></Row>
      <Row label="Hover delay"><Slider label="Hover delay" testId="hover-delay" min={0} max={1000} step={10} value={i.hoverDelayMs} format={(v) => `${v} ms`} onChange={(v) => set({ hoverDelayMs: v })} /></Row>
      <Row label="Collapse delay" hint="How long the island stays open after the pointer leaves."><Slider label="Collapse delay" testId="collapse-delay" min={0} max={3000} step={50} value={i.collapseDelayMs} format={(v) => `${v} ms`} onChange={(v) => set({ collapseDelayMs: v })} /></Row>
      <Row label="Global shortcut" hint="Expands and pins the island, or collapses it. Needs Ctrl, Alt or Win."><HotkeyInput testId="hotkey" value={i.hotkey} onChange={(v) => set({ hotkey: v })} /></Row>
      <h3 className="sub" data-testid="group-widgets">Widgets</h3>
      <Row label="Show timers on the island" hint="While the island is collapsed and nothing else is happening, a running Pomodoro or countdown appears as one line. It never brings back a hidden island.">
        <Toggle label="Show timers on the island" testId="widget-status" checked={i.showWidgetStatus} onChange={(v) => set({ showWidgetStatus: v })} />
      </Row>
      <h3 className="sub" data-testid="group-volume">Volume</h3>
      <Row label="Volume button" hint="Shows a speaker button in the expanded island. It opens a small panel for the Windows volume: slider, plus and minus, mute.">
        <Toggle label="Volume button" testId="volume-button" checked={i.volumeButton} onChange={(v) => set({ volumeButton: v })} />
      </Row>
      <Row label="Volume step" hint="How much the plus and minus buttons change the volume.">
        <Slider label="Volume step" testId="volume-step" min={1} max={20} step={1} value={i.volumeStep} disabled={!i.volumeButton} format={(v) => `${v}%`} onChange={(v) => set({ volumeStep: v })} />
      </Row>
      <ResetButton testId="reset-island" onClick={() => update({ island: null } as unknown as SettingsPatch)} />
    </section>
  );
}

function MusicPage({ s, update }: { s: Settings; update: Update }) {
  const m = s.music;
  const set = (p: Partial<Settings['music']>) => update({ music: p });
  return (
    <section>
      <h2>Music &amp; lyrics</h2>
      <Row label="Show album artwork"><Toggle label="Show album artwork" testId="artwork" checked={m.artwork} onChange={(v) => set({ artwork: v })} /></Row>
      <Row label="Show equalizer" hint="Animates while a track is playing. It reflects play state, not the audio signal."><Toggle label="Show equalizer" testId="visualizer" checked={m.visualizer} onChange={(v) => set({ visualizer: v })} /></Row>
      <Row label="Show lyrics"><Toggle label="Show lyrics" testId="lyrics-enabled" checked={m.lyricsEnabled} onChange={(v) => set({ lyricsEnabled: v })} /></Row>
      <Row label="Lyrics in the collapsed island"><Toggle label="Lyrics in the collapsed island" testId="lyrics-collapsed" checked={m.lyricsInCollapsed} onChange={(v) => set({ lyricsInCollapsed: v })} /></Row>
      <Row label="Lyrics animation" hint="Letters: each letter rises in and lights up as it is sung. Line: the whole line fades in.">
        <Segmented<Settings['music']['lyricsStyle']> label="Lyrics animation" testId="lyrics-style" value={m.lyricsStyle} onChange={(v) => set({ lyricsStyle: v })} options={[{ value: 'letters', label: 'Letters' }, { value: 'line', label: 'Line' }]} />
      </Row>
      <Row label="Word-by-word highlighting" hint="Line style only, and only for lyrics that include per-word timing."><Toggle label="Word-by-word highlighting" testId="karaoke" checked={m.karaoke} onChange={(v) => set({ karaoke: v })} /></Row>
      <Row label="Lyrics font" hint="Any font installed on this PC, with fallbacks separated by commas."><CommitText label="Lyrics font" testId="lyrics-font" width={260} value={m.lyricsFont} onCommit={(v) => set({ lyricsFont: v })} /></Row>
      <Row label="Lyrics size"><Slider label="Lyrics size" testId="lyrics-size" min={12} max={32} step={1} value={m.lyricsFontSize} format={(v) => `${v}px`} onChange={(v) => set({ lyricsFontSize: v })} /></Row>
      <Row label="Lyrics animation speed"><Slider label="Lyrics animation speed" testId="lyrics-speed" min={0.25} max={3} step={0.05} value={m.lyricsAnimationSpeed} format={(v) => `${v.toFixed(2)}×`} onChange={(v) => set({ lyricsAnimationSpeed: v })} /></Row>
      <Row label="Lyrics timing offset" hint="Positive values show each line later."><Slider label="Lyrics timing offset" testId="lyrics-offset" min={-3000} max={3000} step={50} value={m.lyricsOffsetMs} format={(v) => `${v} ms`} onChange={(v) => set({ lyricsOffsetMs: v })} /></Row>
      <Row label="Find lyrics automatically" hint="Looks the playing track up on LRCLIB (lrclib.net). This sends the song title, artist, album and length to that service. Turn off to use only .lrc files you load yourself.">
        <Toggle label="Find lyrics automatically" testId="lyrics-online" checked={m.lyricsOnline} onChange={(v) => set({ lyricsOnline: v })} />
      </Row>
      <ResetButton testId="reset-music" onClick={() => update({ music: null } as unknown as SettingsPatch)} />
    </section>
  );
}

const FACE_STATES: AssistantState[] = ['idle', 'listening', 'thinking', 'speaking', 'error'];

const PROVIDERS: Array<{ value: Settings['ai']['provider']; label: string }> = [
  { value: 'ollama', label: 'Ollama (local, free)' },
  { value: 'openai', label: 'OpenAI' },
  { value: 'gemini', label: 'Google Gemini' },
  { value: 'openrouter', label: 'OpenRouter' },
  { value: 'claude', label: 'Anthropic Claude' },
  { value: 'openai-compatible', label: 'OpenAI-compatible server' },
];

const KEY_HELP: Record<string, string> = {
  openai: 'Create a key at platform.openai.com/api-keys.',
  gemini: 'Create a key in Google AI Studio (aistudio.google.com/apikey).',
  openrouter: 'Create a key at openrouter.ai/keys.',
  claude: 'Create a key in the Anthropic Console (console.anthropic.com).',
  'openai-compatible': 'Only needed if your server asks for one.',
};

const PRIVACY: Record<string, string> = {
  ollama: 'Messages go only to the Ollama address above. If that is this PC, nothing leaves it.',
  openai: 'Your messages are sent to OpenAI.',
  gemini: 'Your messages are sent to Google.',
  openrouter: 'Your messages are sent to OpenRouter, which forwards them to the model you choose.',
  claude: 'Your messages are sent to Anthropic.',
  'openai-compatible': 'Your messages are sent to the server address above.',
};

function AssistantPage({ s, update }: { s: Settings; update: Update }) {
  const backend = useBackend();
  const [shown, setShown] = useState<AssistantState>('idle');
  const preview = (st: AssistantState) => {
    setShown(st);
    void backend.assistantPreview(st);
  };
  useEffect(() => () => void backend.assistantPreview('idle'), [backend]);

  const ai = s.ai;
  const p = ai.provider;
  const model = ai.models[p] ?? '';
  const usesKey = p !== 'ollama';
  const [keySaved, setKeySaved] = useState(false);
  const [keyInput, setKeyInput] = useState('');
  const [keyError, setKeyError] = useState<string | null>(null);
  const [test, setTest] = useState<AiTestResult | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [clearMsg, setClearMsg] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  const runTest = async () => {
    setBusy(true);
    try {
      const r = await backend.aiTest(p);
      setTest(r);
      if (r.models.length) setModels(r.models);
      else if (!r.ok) setModels([]);
      // OpenRouter's connection check does not list models; fetch them separately when it passed.
      if (r.ok && p === 'openrouter') setModels(await backend.aiListModels(p).catch(() => []));
    } catch (e) {
      setTest({ ok: false, message: String(e instanceof Error ? e.message : e), models: [], local: false });
    } finally {
      setBusy(false);
    }
  };

  // When the provider changes: forget the previous provider's results, read the key status, and
  // look for a local Ollama straight away (that request never leaves this PC).
  useEffect(() => {
    setTest(null);
    setModels([]);
    setKeyInput('');
    setKeyError(null);
    backend.aiKeyStatus(p).then(setKeySaved, () => setKeySaved(false));
    if (p === 'ollama') void runTest();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p, backend]);

  const saveKey = async () => {
    setKeyError(null);
    try {
      await backend.aiSetKey(p, keyInput);
      setKeyInput('');
      setKeySaved(true);
      await runTest(); // validate the key straight away
    } catch (e) {
      setKeyError(e instanceof Error ? e.message : String(e));
    }
  };
  const clearKey = async () => {
    await backend.aiClearKey(p);
    setKeySaved(false);
    setTest(null);
    setModels([]);
  };
  const setModel = (v: string) => update({ ai: { models: { [p]: v } } });
  const withCurrent = model && !models.includes(model) ? [model, ...models] : models;

  return (
    <section>
      <h2>Assistant</h2>
      <Row label="Preview the assistant face" hint="Plays the chosen state on the real island.">
        <div className="face-preview">
          <AssistantFace state={shown} size={96} />
          <Segmented<AssistantState> label="Assistant state" testId="face-state" value={shown} onChange={preview} options={FACE_STATES.map((v) => ({ value: v, label: v[0].toUpperCase() + v.slice(1) }))} />
        </div>
      </Row>

      <h3>AI provider</h3>
      <Row label="Provider" hint="Ollama runs models on your own PC with no account or key.">
        <Select<Settings['ai']['provider']> label="AI provider" testId="ai-provider" value={p} onChange={(v) => update({ ai: { provider: v } })} options={PROVIDERS} />
      </Row>

      {p === 'ollama' && (
        <Row label="Ollama address" hint="Where Ollama is listening. The default is this PC.">
          <CommitText label="Ollama address" testId="ollama-endpoint" width={260} value={ai.ollamaEndpoint} onCommit={(v) => update({ ai: { ollamaEndpoint: v } })} />
        </Row>
      )}
      {p === 'ollama' && (
        <>
          <Row label="Let the model think first" hint="Reasoning models (such as Qwen) can spend a long time on hidden thinking before the first word. Off is much faster; turn on for harder questions.">
            <Toggle label="Let the model think first" testId="ollama-think" checked={ai.ollamaThink} onChange={(v) => update({ ai: { ollamaThink: v } })} />
          </Row>
          <Row label="Keep model loaded" hint="How long Ollama keeps the model in memory after a reply. Longer avoids waiting for it to load again.">
            <Select<string> label="Keep model loaded" testId="ollama-keepalive" value={ai.ollamaKeepAlive} onChange={(v) => update({ ai: { ollamaKeepAlive: v } })} options={keepCurrent(KEEP_ALIVE_OPTIONS, ai.ollamaKeepAlive)} />
          </Row>
          <Row label="Context window" hint="How much conversation the model reads each time. Smaller is faster and uses less memory (important on a 4 GB graphics card).">
            <Select<number> label="Context window" testId="ollama-ctx" value={ai.ollamaNumCtx} onChange={(v) => update({ ai: { ollamaNumCtx: v } })} options={keepCurrent(CTX_OPTIONS, ai.ollamaNumCtx)} />
          </Row>
        </>
      )}
      {p === 'openai-compatible' && (
        <Row label="Server address" hint="The base URL including /v1, e.g. http://localhost:1234/v1 for LM Studio.">
          <CommitText label="Server address" testId="compat-endpoint" width={260} placeholder="http://localhost:1234/v1" value={ai.compatibleBaseUrl} onCommit={(v) => update({ ai: { compatibleBaseUrl: v } })} />
        </Row>
      )}

      {usesKey && (
        <Row label="API key" hint={`${KEY_HELP[p] ?? ''} Stored in Windows Credential Manager, never in VEX's settings file.`}>
          <div className="key-control">
            <div className="key-state" data-testid="key-state" data-saved={keySaved}>{keySaved ? '● Key saved' : '○ No key saved'}</div>
            <div className="key-row">
              <input type="password" autoComplete="off" spellCheck={false} aria-label="API key" data-testid="ai-key" placeholder={keySaved ? 'Paste a new key to replace it' : 'Paste your key'} value={keyInput} onChange={(e) => setKeyInput(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && keyInput.trim() && void saveKey()} />
              <button type="button" className="btn" disabled={!keyInput.trim()} data-testid="save-key" onClick={() => void saveKey()}>Save key</button>
              <button type="button" className="btn danger" disabled={!keySaved} data-testid="clear-key" onClick={() => void clearKey()}>Clear</button>
            </div>
            {keyError && <div className="status-line bad" role="alert" data-testid="key-error">{keyError}</div>}
          </div>
        </Row>
      )}

      <Row label="Connection" hint={PRIVACY[p]}>
        <button type="button" className="btn" disabled={busy} data-testid="test-connection" onClick={() => void runTest()}>{busy ? 'Testing…' : 'Test connection'}</button>
      </Row>
      {test && (
        <div className="status-line" data-ok={test.ok} role="status" data-testid="test-result">
          {test.ok ? '✓ ' : '✕ '}{test.message}
        </div>
      )}

      <Row label="Model" hint={p === 'ollama' ? 'Only models already installed in Ollama are listed. VEX never downloads models.' : 'Pick from the list after testing, or type a model name.'}>
        {p === 'ollama' ? (
          <Select<string> label="Model" testId="ai-model" value={model} onChange={setModel} options={[{ value: '', label: models.length ? 'Choose a model…' : 'No models found' }, ...withCurrent.map((m) => ({ value: m, label: m }))]} />
        ) : withCurrent.length > 0 ? (
          <Select<string> label="Model" testId="ai-model" value={model} onChange={setModel} options={[{ value: '', label: 'Choose a model…' }, ...withCurrent.map((m) => ({ value: m, label: m }))]} />
        ) : (
          <CommitText label="Model" testId="ai-model-text" width={240} placeholder="model name" value={model} onCommit={setModel} />
        )}
      </Row>

      <Row label="System prompt" hint="The instructions the assistant always receives. The default asks for accurate, properly explained answers and tells it how to use the music you share.">
        <div className="prompt-control">
        <textarea aria-label="System prompt" data-testid="ai-prompt" rows={3} defaultValue={ai.systemPrompt} key={ai.systemPrompt} onBlur={(e) => e.target.value !== ai.systemPrompt && update({ ai: { systemPrompt: e.target.value } })} />
        <button type="button" className="link-btn" data-testid="reset-prompt" onClick={() => void update({ ai: { systemPrompt: null } } as unknown as SettingsPatch)}>Reset to default</button>
        </div>
      </Row>

      <h3>Privacy and history</h3>
      <Row label="Share music only when I ask" hint="The assistant sees the current song or lyrics only for a message that asks about them (for example “What song is playing?”). Turn off to share only with the buttons in the chat.">
        <Toggle label="Share music only when I ask" testId="ctx-on-request" checked={ai.contextOnRequest} onChange={(v) => update({ ai: { contextOnRequest: v } })} />
      </Row>
      <Row label="Chat history" hint="Saved on this PC only (chats.json in VEX's data folder). Only the current chat is sent to your provider, and only the most recent messages of it.">
        {confirmClear ? (
          <span className="confirm">
            <button type="button" className="btn danger" data-testid="confirm-clear-chats" onClick={async () => { await backend.chatClearAll(); setConfirmClear(false); setClearMsg('Chat history cleared.'); }}>Delete all chats</button>
            <button type="button" className="btn" onClick={() => setConfirmClear(false)}>Cancel</button>
          </span>
        ) : (
          <button type="button" className="btn" data-testid="clear-chats" onClick={() => { setClearMsg(null); setConfirmClear(true); }}>Clear chat history…</button>
        )}
      </Row>
      {clearMsg && <div className="status-line" data-ok="true" role="status" data-testid="clear-msg">{clearMsg}</div>}
      <ResetButton testId="reset-ai" onClick={() => update({ ai: null } as unknown as SettingsPatch)} />
    </section>
  );
}

function AboutPage() {
  return (
    <section>
      <h2>About VEX</h2>
      <p>VEX is a free Dynamic Island for Windows. Island shell, live Windows media controls, automatic synced lyrics, and an AI assistant (Ollama, OpenAI, Gemini, OpenRouter, Claude).</p>
      <p className="dim">VEX is an independent project. It is not affiliated with Notchify, Fenn or any other app, and uses none of their code, names or artwork. Settings are stored locally in your user profile. VEX only uses the network for two optional things: the lyrics lookup on LRCLIB (song title, artist, album and length; off in Music & lyrics) and the chat messages you send to the AI provider you choose. A local Ollama keeps chats on this PC. API keys are kept in Windows Credential Manager.</p>
      <h3>Roadmap</h3>
      <ul>
                <li>Milestone 5–7 — AI providers, voice, system actions</li>
      </ul>
    </section>
  );
}
