import { useResolvedTheme } from '../backend/hooks';
import { DEFAULT_ACCENT } from '../island/accent';
import type { Settings, SettingsPatch } from '../shared/types';
import { Row, Segmented, Slider } from './controls';
import { IslandPreview } from './IslandPreview';
import { activePreset, PRESETS } from './presets';

const pct = (v: number) => `${Math.round(v * 100)}%`;

export function AppearancePage({ s, update }: { s: Settings; update(p: SettingsPatch): Promise<void> }) {
  const a = s.appearance;
  const set = (p: Partial<Settings['appearance']>) => update({ appearance: p });
  const theme = useResolvedTheme(a.theme);
  // With the artwork source the real island follows the cover; the preview shows the fallback colour.
  const accent = a.accentSource === 'custom' ? a.customAccent : a.customAccent || DEFAULT_ACCENT;
  const current = activePreset(a);

  return (
    <section className="studio" data-testid="appearance-studio">
      <h2>Appearance</h2>
      <div className="studio-preview">
        <IslandPreview a={a} theme={theme} accent={accent} />
        <p className="preview-note">Live preview. {a.accentSource === 'artwork' ? 'While music plays, the accent follows the album cover.' : 'The accent stays as chosen.'}</p>
      </div>

      <h3>Presets</h3>
      <div className="presets" role="group" aria-label="Theme presets">
        {PRESETS.map((p) => (
          <button key={p.id} className="preset" data-active={current === p.id} aria-pressed={current === p.id} data-testid={`preset-${p.id}`} title={p.blurb} onClick={() => void set(p.look)}>
            <span className="preset-swatch" data-theme={p.look.theme} data-shape={p.look.shape} style={{ ['--sw' as string]: p.look.customAccent }}><i /></span>
            <span className="preset-name">{p.name}</span>
          </button>
        ))}
      </div>

      <h3>Surface</h3>
      <div className="group">
        <Row label="Theme"><Segmented label="Theme" testId="theme" value={a.theme} onChange={(v) => set({ theme: v })} options={[{ value: 'dark', label: 'Dark' }, { value: 'light', label: 'Light' }, { value: 'system', label: 'System' }]} /></Row>
        <Row label="Shape" hint="Pill floats below the top edge; Notch hangs from it with a flat top."><Segmented label="Shape" testId="shape" value={a.shape} onChange={(v) => set({ shape: v })} options={[{ value: 'pill', label: 'Pill' }, { value: 'notch', label: 'Notch' }]} /></Row>
        <Row label="Glass opacity" hint="Lower is lighter, but the island never goes below about 90% opaque so text stays readable."><Slider label="Glass opacity" testId="glass" min={0.4} max={1} step={0.01} value={a.glassOpacity} format={pct} onChange={(v) => set({ glassOpacity: v })} /></Row>
        <Row label="Corner radius" hint="Applies to the expanded island."><Slider label="Corner radius" testId="radius" min={8} max={40} step={1} value={a.cornerRadius} format={(v) => `${v}px`} onChange={(v) => set({ cornerRadius: v })} /></Row>
        <Row label="Size" hint="Scales every part of the island."><Slider label="Size" testId="scale" min={0.8} max={1.4} step={0.05} value={a.sizeScale} format={pct} onChange={(v) => set({ sizeScale: v })} /></Row>
      </div>

      <h3>Colour</h3>
      <div className="group">
        <Row label="Accent colour" hint="Artwork uses the dominant colour of the current album cover."><Segmented label="Accent source" testId="accent-source" value={a.accentSource} onChange={(v) => set({ accentSource: v })} options={[{ value: 'artwork', label: 'Album artwork' }, { value: 'custom', label: 'Custom' }]} /></Row>
        <Row label="Custom accent" hint="Used when the source is Custom, or when a track has no artwork.">
          <input type="color" aria-label="Custom accent colour" data-testid="accent-color" value={a.customAccent} onChange={(e) => set({ customAccent: e.target.value })} />
        </Row>
      </div>

      <h3>Motion</h3>
      <div className="group">
        <Row label="Animation" hint="Off removes all motion. Your Windows 'reduce motion' setting is also respected."><Segmented label="Animation" testId="motion" value={a.motion} onChange={(v) => set({ motion: v })} options={[{ value: 'off', label: 'Off' }, { value: 'subtle', label: 'Subtle' }, { value: 'normal', label: 'Normal' }, { value: 'bouncy', label: 'Bouncy' }]} /></Row>
      </div>

      <button className="reset" onClick={() => update({ appearance: null } as unknown as SettingsPatch)} data-testid="reset-appearance">Reset section</button>
    </section>
  );
}
