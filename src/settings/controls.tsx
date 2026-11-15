import { useEffect, useId, useRef, useState, type ReactNode } from 'react';

export function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="row">
      <div className="row-text">
        <div className="row-label">{label}</div>
        {hint && <div className="row-hint">{hint}</div>}
      </div>
      <div className="row-control">{children}</div>
    </div>
  );
}

export function Toggle({ checked, onChange, label, testId }: { checked: boolean; onChange(v: boolean): void; label: string; testId?: string }) {
  return (
    <button role="switch" aria-checked={checked} aria-label={label} className="toggle" data-on={checked} data-testid={testId} onClick={() => onChange(!checked)}>
      <span />
    </button>
  );
}

export function Slider({
  value, min, max, step, onChange, format, label, testId,
}: { value: number; min: number; max: number; step: number; onChange(v: number): void; format?: (v: number) => string; label: string; testId?: string }) {
  return (
    <div className="slider">
      <input type="range" aria-label={label} min={min} max={max} step={step} value={value} data-testid={testId} onChange={(e) => onChange(Number(e.target.value))} />
      <span className="slider-val">{format ? format(value) : value}</span>
    </div>
  );
}

export function Select<T extends string | number>({
  value, options, onChange, label, testId,
}: { value: T; options: Array<{ value: T; label: string }>; onChange(v: T): void; label: string; testId?: string }) {
  return (
    <select
      aria-label={label}
      value={String(value)}
      data-testid={testId}
      onChange={(e) => {
        const o = options.find((x) => String(x.value) === e.target.value);
        if (o) onChange(o.value);
      }}
    >
      {options.map((o) => <option key={String(o.value)} value={String(o.value)}>{o.label}</option>)}
    </select>
  );
}

export function Segmented<T extends string>({
  value, options, onChange, label, testId,
}: { value: T; options: Array<{ value: T; label: string }>; onChange(v: T): void; label: string; testId?: string }) {
  const id = useId();
  return (
    <div className="segmented" role="radiogroup" aria-label={label} data-testid={testId}>
      {options.map((o) => (
        <button key={o.value} id={`${id}-${o.value}`} role="radio" aria-checked={value === o.value} data-on={value === o.value} data-testid={testId ? `${testId}-${o.value}` : undefined} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** Text input that commits on blur/Enter, so typing a half-finished value never reaches the backend. */
export function CommitText({
  value, onCommit, label, testId, placeholder, width,
}: { value: string; onCommit(v: string): void; label: string; testId?: string; placeholder?: string; width?: number }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = () => draft !== value && onCommit(draft);
  return (
    <input
      type="text" aria-label={label} data-testid={testId} placeholder={placeholder} value={draft} style={width ? { width } : undefined}
      onChange={(e) => setDraft(e.target.value)} onBlur={commit}
      onKeyDown={(e) => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); if (e.key === 'Escape') setDraft(value); }}
    />
  );
}

const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta']);

/** Press-to-record global shortcut field. Produces accelerators like "Ctrl+Alt+I". */
export function HotkeyInput({ value, onChange, testId }: { value: string; onChange(v: string): void; testId?: string }) {
  const [recording, setRecording] = useState(false);
  const ref = useRef<HTMLButtonElement>(null);
  return (
    <button
      ref={ref}
      className="hotkey"
      data-recording={recording}
      data-testid={testId}
      aria-label={`Global shortcut: ${value}. Press to change.`}
      onClick={() => setRecording(true)}
      onBlur={() => setRecording(false)}
      onKeyDown={(e) => {
        if (!recording) return;
        e.preventDefault();
        if (e.key === 'Escape') return setRecording(false);
        if (MODIFIER_KEYS.has(e.key)) return;
        if (!(e.ctrlKey || e.altKey || e.metaKey)) return; // need Ctrl/Alt/Win so it never hijacks plain typing
        const key = e.code.startsWith('Key') ? e.code.slice(3) : e.code.startsWith('Digit') ? e.code.slice(5) : e.key.length === 1 ? e.key.toUpperCase() : e.code;
        const parts = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super', key].filter(Boolean) as string[];
        setRecording(false);
        ref.current?.blur();
        onChange(parts.join('+'));
      }}
    >
      {recording ? 'Press a shortcut…' : value}
    </button>
  );
}
