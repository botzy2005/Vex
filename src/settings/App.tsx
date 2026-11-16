import { useEffect, useState } from 'react';
import { AssistantFace } from '../assistant/AssistantFace';
import { useBackend, useResolvedTheme, useSettings } from '../backend/hooks';
import type { AssistantState, MonitorInfo, Settings, SettingsPatch } from '../shared/types';
import { CommitText, HotkeyInput, Row, Segmented, Select, Slider, Toggle } from './controls';

type Page = 'appearance' | 'island' | 'music' | 'assistant' | 'about';
const PAGES: Array<{ id: Page; label: string }> = [
  { id: 'appearance', label: 'Appearance' },
  { id: 'island', label: 'Island' },
  { id: 'music', label: 'Music & lyrics' },
  { id: 'assistant', label: 'Assistant' },
  { id: 'about', label: 'About' },
];

type Update = (p: SettingsPatch) => Promise<void>;
const pct = (v: number) => `${Math.round(v * 100)}%`;

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
        <h1>Islet</h1>
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
        {page === 'about' && <AboutPage />}
      </main>
    </div>
  );
}

function ResetButton({ onClick, testId }: { onClick(): void; testId?: string }) {
  return <button className="reset" onClick={onClick} data-testid={testId}>Reset section</button>;
}

function AppearancePage({ s, update }: { s: Settings; update: Update }) {
  const a = s.appearance;
  const set = (p: Partial<Settings['appearance']>) => update({ appearance: p });
  return (
    <section>
      <h2>Appearance</h2>
      <Row label="Theme"><Segmented label="Theme" testId="theme" value={a.theme} onChange={(v) => set({ theme: v })} options={[{ value: 'dark', label: 'Dark' }, { value: 'light', label: 'Light' }, { value: 'system', label: 'System' }]} /></Row>
      <Row label="Shape" hint="Pill floats below the top edge; Notch hangs from it with a flat top."><Segmented label="Shape" testId="shape" value={a.shape} onChange={(v) => set({ shape: v })} options={[{ value: 'pill', label: 'Pill' }, { value: 'notch', label: 'Notch' }]} /></Row>
      <Row label="Glass opacity" hint="Lower values let the desktop show through."><Slider label="Glass opacity" testId="glass" min={0.4} max={1} step={0.01} value={a.glassOpacity} format={pct} onChange={(v) => set({ glassOpacity: v })} /></Row>
      <Row label="Corner radius" hint="Applies to the expanded island."><Slider label="Corner radius" testId="radius" min={8} max={40} step={1} value={a.cornerRadius} format={(v) => `${v}px`} onChange={(v) => set({ cornerRadius: v })} /></Row>
      <Row label="Size" hint="Scales every part of the island."><Slider label="Size" testId="scale" min={0.8} max={1.4} step={0.05} value={a.sizeScale} format={pct} onChange={(v) => set({ sizeScale: v })} /></Row>
      <Row label="Animation" hint="Off removes all motion. Your Windows 'reduce motion' setting is also respected."><Segmented label="Animation" testId="motion" value={a.motion} onChange={(v) => set({ motion: v })} options={[{ value: 'off', label: 'Off' }, { value: 'subtle', label: 'Subtle' }, { value: 'normal', label: 'Normal' }, { value: 'bouncy', label: 'Bouncy' }]} /></Row>
      <Row label="Accent colour" hint="Artwork uses the dominant colour of the current album cover."><Segmented label="Accent source" testId="accent-source" value={a.accentSource} onChange={(v) => set({ accentSource: v })} options={[{ value: 'artwork', label: 'Album artwork' }, { value: 'custom', label: 'Custom' }]} /></Row>
      <Row label="Custom accent" hint="Used when the source is Custom, or when a track has no artwork.">
        <input type="color" aria-label="Custom accent colour" data-testid="accent-color" value={a.customAccent} onChange={(e) => set({ customAccent: e.target.value })} />
      </Row>
      <ResetButton testId="reset-appearance" onClick={() => update({ appearance: null } as unknown as SettingsPatch)} />
    </section>
  );
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
      <Row label="Display" hint="Which monitor the island appears on.">
        <Select<number> label="Display" testId="monitor" value={i.monitorIndex} onChange={(v) => set({ monitorIndex: v })}
          options={[{ value: -1, label: 'Primary display' }, ...monitors.map((m) => ({ value: m.index, label: `${m.name} (${m.width}×${m.height} @ ${Math.round(m.scale * 100)}%)` }))]} />
      </Row>
      <Row label="Horizontal offset" hint="Shift the island left or right of centre."><Slider label="Horizontal offset" testId="offset" min={-800} max={800} step={5} value={i.offsetX} format={(v) => `${v}px`} onChange={(v) => set({ offsetX: v })} /></Row>
      <Row label="Gap from top edge"><Slider label="Gap from top edge" testId="margin" min={0} max={60} step={1} value={i.marginTop} format={(v) => `${v}px`} onChange={(v) => set({ marginTop: v })} /></Row>
      <Row label="Expand on hover" hint="When off, click the island to expand it."><Toggle label="Expand on hover" testId="hover" checked={i.hoverToExpand} onChange={(v) => set({ hoverToExpand: v })} /></Row>
      <Row label="Hover delay"><Slider label="Hover delay" testId="hover-delay" min={0} max={1000} step={10} value={i.hoverDelayMs} format={(v) => `${v} ms`} onChange={(v) => set({ hoverDelayMs: v })} /></Row>
      <Row label="Collapse delay" hint="How long the island stays open after the pointer leaves."><Slider label="Collapse delay" testId="collapse-delay" min={0} max={3000} step={50} value={i.collapseDelayMs} format={(v) => `${v} ms`} onChange={(v) => set({ collapseDelayMs: v })} /></Row>
      <Row label="Global shortcut" hint="Expands and pins the island, or collapses it. Needs Ctrl, Alt or Win."><HotkeyInput testId="hotkey" value={i.hotkey} onChange={(v) => set({ hotkey: v })} /></Row>
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

function AssistantPage({ s, update }: { s: Settings; update: Update }) {
  const backend = useBackend();
  const [shown, setShown] = useState<AssistantState>('idle');
  const preview = (st: AssistantState) => {
    setShown(st);
    void backend.assistantPreview(st);
  };
  useEffect(() => () => void backend.assistantPreview('idle'), [backend]);
  const ai = s.ai;
  return (
    <section>
      <h2>Assistant</h2>
      <div className="callout">The assistant's model connection, voice and computer controls arrive in Milestones 5–7. Here you can preview how it looks on the island.</div>
      <Row label="Preview the assistant face" hint="Plays the chosen state on the real island.">
        <div className="face-preview">
          <AssistantFace state={shown} size={96} />
          <Segmented<AssistantState> label="Assistant state" testId="face-state" value={shown} onChange={preview} options={FACE_STATES.map((v) => ({ value: v, label: v[0].toUpperCase() + v.slice(1) }))} />
        </div>
      </Row>
      <Row label="Provider" hint="Saved now; connected in Milestone 5. Local Ollama needs no account or key.">
        <Select<Settings['ai']['provider']> label="AI provider" testId="ai-provider" value={ai.provider} onChange={(v) => update({ ai: { provider: v } })}
          options={[
            { value: 'ollama', label: 'Ollama (local, free)' }, { value: 'openai-compatible', label: 'OpenAI-compatible server' },
            { value: 'openrouter', label: 'OpenRouter' }, { value: 'gemini', label: 'Google Gemini' },
            { value: 'claude', label: 'Anthropic Claude' }, { value: 'openai', label: 'OpenAI' },
          ]} />
      </Row>
      <Row label="Model" hint="Model name as the provider expects it, e.g. llama3.2."><CommitText label="Model" testId="ai-model" width={220} placeholder="e.g. llama3.2" value={ai.model} onCommit={(v) => update({ ai: { model: v } })} /></Row>
      <Row label="System prompt">
        <textarea aria-label="System prompt" data-testid="ai-prompt" rows={3} defaultValue={ai.systemPrompt} key={ai.systemPrompt} onBlur={(e) => e.target.value !== ai.systemPrompt && update({ ai: { systemPrompt: e.target.value } })} />
      </Row>
      <ResetButton testId="reset-ai" onClick={() => update({ ai: null } as unknown as SettingsPatch)} />
    </section>
  );
}

function AboutPage() {
  return (
    <section>
      <h2>About Islet</h2>
      <p>Islet is a free Dynamic Island for Windows. Island shell, live Windows media controls, automatic synced lyrics, assistant face and settings.</p>
      <p className="dim">Islet is an independent project. It is not affiliated with Notchify, Fenn or any other app, and uses none of their code, names or artwork. Settings are stored locally in your user profile. The only network request Islet makes is the optional lyrics lookup on LRCLIB (song title, artist, album and length), which you can turn off in Music & lyrics.</p>
      <h3>Roadmap</h3>
      <ul>
                <li>Milestone 5–7 — AI providers, voice, system actions</li>
      </ul>
    </section>
  );
}
