import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import { useBackend, useResolvedTheme, useSettings } from '../backend/hooks';
import { DEFAULT_ACCENT, onAccent } from '../island/accent';
import type { AiEvent, LauncherAiInfo, LauncherAction, LauncherRow, LauncherSearchResult } from '../shared/types';
import { kindLabel, RowGlyph, SearchIcon, Spinner, StarIcon } from './icons';

const EMPTY: LauncherSearchResult = { rows: [], appsLoading: false, filesLoading: false };
const EXIT_MS = 120;
const DEBOUNCE_MS = 35;
/** Ignore the blur that Windows can deliver right as the window is being shown. */
const BLUR_GRACE_MS = 300;

type Phase = 'closed' | 'open' | 'closing';
type Status = { kind: 'error' | 'info'; text: string } | null;

interface AiState {
  kind: string;
  text: string;
  requestId: number | null;
  answer: string;
  state: 'sending' | 'streaming' | 'done' | 'error' | 'cancelled';
  error: string | null;
}

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Icons are requested once per app and remembered for the life of the page. */
const iconCache = new Map<string, string | null>();

function AppIcon({ id, name, kind }: { id: string; name: string; kind: 'desktop' | 'store' | null }) {
  const backend = useBackend();
  const [src, setSrc] = useState<string | null>(iconCache.get(id) ?? null);
  useEffect(() => {
    let live = true;
    if (iconCache.has(id)) {
      setSrc(iconCache.get(id) ?? null);
      return;
    }
    backend
      .launcherAppIcon(id)
      .then((url) => {
        iconCache.set(id, url);
        if (live) setSrc(url);
      })
      .catch(() => iconCache.set(id, null));
    return () => {
      live = false;
    };
  }, [backend, id]);
  if (src) return <img className="lx-appicon" src={src} alt="" draggable={false} />;
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) % 360;
  const letter = (name.match(/[\p{L}\p{N}]/u)?.[0] ?? '?').toUpperCase();
  return (
    <span className="lx-tile" data-kind={kind ?? 'desktop'} style={{ '--tile-h': h } as CSSProperties} aria-hidden="true">
      {letter}
    </span>
  );
}

function localTime(): string {
  return new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}
function localDate(): string {
  return new Date().toLocaleDateString(undefined, { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' });
}

/** What the row shows, with the page-side values (clock, AI readiness) filled in. */
function present(row: LauncherRow, ai: LauncherAiInfo | null): LauncherRow {
  const a = row.target.type === 'command' ? row.target.action : null;
  if (a?.type === 'time') return { ...row, title: localTime(), subtitle: 'Current time · Enter to copy' };
  if (a?.type === 'date') return { ...row, title: localDate(), subtitle: "Today's date · Enter to copy" };
  if (a?.type === 'calc' && row.available) return { ...row, subtitle: `${a.expression} = · Enter to copy` };
  if (a?.type === 'ai' && ai && !ai.ready) return { ...row, available: false, reason: ai.problem ?? 'AI is not set up.' };
  return row;
}

const AI_NAMES: Record<string, string> = { ask: 'Ask', explain: 'Explain', summarize: 'Summarise', rewrite: 'Rewrite', translate: 'Translate' };

export function LauncherApp() {
  const backend = useBackend();
  const { settings } = useSettings();
  const theme = useResolvedTheme(settings?.appearance.theme);

  const [phase, setPhase] = useState<Phase>(backend.kind === 'tauri' ? 'closed' : 'open');
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<LauncherSearchResult>(EMPTY);
  /** The text the current rows were produced for. Rows are always run against this, never a newer query. */
  const [resultQuery, setResultQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const [status, setStatus] = useState<Status>(null);
  const [aiInfo, setAiInfo] = useState<LauncherAiInfo | null>(null);
  const [ai, setAi] = useState<AiState | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const seq = useRef(0);
  const queryRef = useRef('');
  const phaseRef = useRef<Phase>(phase);
  const closeTimer = useRef<number | undefined>(undefined);
  const openedAt = useRef(0);
  const pendingEnter = useRef(false);
  const aiRef = useRef<AiState | null>(null);
  aiRef.current = ai;
  queryRef.current = query;
  phaseRef.current = phase;

  const a = settings?.appearance;
  const accent = a?.accentSource === 'custom' && a.customAccent ? a.customAccent : DEFAULT_ACCENT;
  const reduced = a?.motion === 'off';

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const rows = useMemo(() => result.rows.map((r) => present(r, aiInfo)), [result.rows, aiInfo]);

  // ---- searching: debounced, newest answer wins, rows remember the text that produced them
  const search = useCallback(
    (q: string) => {
      const n = ++seq.current;
      backend
        .launcherSearch(q)
        .then((r) => {
          if (n !== seq.current) return;
          setResult(r);
          setResultQuery(q);
          setSelected((s) => Math.min(s, Math.max(0, r.rows.length - 1)));
        })
        .catch((e) => {
          if (n === seq.current) setStatus({ kind: 'error', text: errText(e) });
        });
    },
    [backend],
  );

  useEffect(() => {
    const t = window.setTimeout(() => search(query), query.trim() ? DEBOUNCE_MS : 0);
    return () => window.clearTimeout(t);
  }, [query, search, settings?.launcher]);

  // Enter pressed while the answer for the newest text was still on its way: run its first row on arrival.
  useEffect(() => {
    if (!pendingEnter.current || resultQuery !== query || rows.length === 0) return;
    pendingEnter.current = false;
    void run(rows[0]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resultQuery, rows]);

  // ---- window lifecycle
  const reset = useCallback(() => {
    pendingEnter.current = false;
    setQuery('');
    setSelected(0);
    setStatus(null);
    setAi(null);
    search('');
    backend.launcherAiInfo().then(setAiInfo).catch(() => setAiInfo(null));
  }, [backend, search]);

  const open = useCallback(() => {
    window.clearTimeout(closeTimer.current);
    openedAt.current = Date.now();
    reset();
    setPhase('open');
    requestAnimationFrame(() => inputRef.current?.focus());
  }, [reset]);

  const close = useCallback(() => {
    if (phaseRef.current !== 'open') return;
    const current = aiRef.current;
    if (current && (current.state === 'sending' || current.state === 'streaming')) void backend.aiCancel().catch(() => undefined);
    setPhase('closing');
    window.clearTimeout(closeTimer.current);
    closeTimer.current = window.setTimeout(
      () => {
        setPhase('closed');
        void backend.launcherHide().catch(() => undefined);
      },
      reduced ? 0 : EXIT_MS,
    );
  }, [backend, reduced]);

  useEffect(() => {
    const subs = [backend.onLauncherOpen(open), backend.onLauncherClose(close), backend.onLauncherIndex(() => search(queryRef.current))];
    return () => {
      subs.forEach((p) => void p.then((off) => off()));
    };
  }, [backend, open, close, search]);

  useEffect(() => {
    if (phase === 'open') requestAnimationFrame(() => inputRef.current?.focus());
  }, [phase]);

  // Clicking away (focus lost) closes, like Spotlight. Only in the real window: browser test pages lose focus freely.
  useEffect(() => {
    if (backend.kind !== 'tauri') return;
    const onBlur = () => {
      if (Date.now() - openedAt.current > BLUR_GRACE_MS) close();
    };
    window.addEventListener('blur', onBlur);
    return () => window.removeEventListener('blur', onBlur);
  }, [backend, close]);

  useEffect(() => () => window.clearTimeout(closeTimer.current), []);

  // ---- AI answers
  useEffect(() => {
    const p = backend.onLauncherAi((e: AiEvent) => {
      const cur = aiRef.current;
      if (!cur) return;
      if (cur.requestId === null) {
        if (e.kind === 'cancelled') return;
        cur.requestId = e.requestId;
      }
      if (e.requestId !== cur.requestId) return;
      setAi((s) => {
        if (!s) return s;
        switch (e.kind) {
          case 'delta':
            return { ...s, state: 'streaming', answer: s.answer + (e.text ?? '') };
          case 'done':
            return { ...s, state: 'done' };
          case 'cancelled':
            return { ...s, state: 'cancelled' };
          case 'error':
            return { ...s, state: 'error', error: e.message ?? 'The AI request failed.' };
          default:
            return s;
        }
      });
    });
    return () => void p.then((off) => off());
  }, [backend]);

  const startAi = useCallback(
    async (action: Extract<LauncherAction, { type: 'ai' }>) => {
      setStatus(null);
      setAi({ kind: action.kind, text: action.text, requestId: null, answer: '', state: 'sending', error: null });
      try {
        const id = await backend.launcherAiSend(action.kind, action.text, action.language);
        setAi((s) => (s && s.requestId === null ? { ...s, requestId: id } : s));
      } catch (e) {
        setAi((s) => (s ? { ...s, state: 'error', error: errText(e) } : s));
      }
    },
    [backend],
  );

  const stopAi = useCallback(() => {
    const cur = aiRef.current;
    if (cur && (cur.state === 'sending' || cur.state === 'streaming')) void backend.aiCancel().catch(() => undefined);
    setAi(null);
    requestAnimationFrame(() => inputRef.current?.focus());
  }, [backend]);

  // ---- running rows
  const copy = useCallback(async (text: string, what: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setStatus({ kind: 'info', text: `${what} copied` });
      return true;
    } catch {
      setStatus({ kind: 'error', text: 'Could not copy to the clipboard.' });
      return false;
    }
  }, []);

  const run = useCallback(
    async (shown: LauncherRow, reveal = false) => {
      if (!shown.available) {
        setStatus({ kind: 'error', text: shown.reason ?? 'That is not available.' });
        return;
      }
      const action = shown.target.type === 'command' ? shown.target.action : null;
      if (action?.type === 'ai') return void startAi(action);
      if (action?.type === 'calc') {
        if (await copy(action.result, 'Result')) window.setTimeout(close, 450);
        return;
      }
      if (action?.type === 'time' || action?.type === 'date') {
        if (await copy(shown.title, action.type === 'time' ? 'Time' : 'Date')) window.setTimeout(close, 450);
        return;
      }
      // The row produced by the typed text: run it against exactly that text.
      const typed = resultQuery;
      try {
        const out = await backend.launcherRun(typed, shown.target, reveal);
        if (typed.trim()) void backend.launcherRecordSearch(typed.trim()).catch(() => undefined);
        if (out.message) {
          setStatus({ kind: 'info', text: out.message });
          window.setTimeout(close, 1400);
        } else close();
      } catch (e) {
        setStatus({ kind: 'error', text: errText(e) });
        search(queryRef.current); // the item may have been removed from the cache
      }
    },
    [backend, close, copy, resultQuery, search, startAi],
  );

  const togglePin = useCallback(
    async (row: LauncherRow) => {
      if (row.target.type !== 'app') return;
      try {
        await backend.launcherSetPin(row.target.id, !row.pinned);
        setStatus({ kind: 'info', text: row.pinned ? `Unpinned ${row.title}` : `Pinned ${row.title}` });
      } catch (e) {
        setStatus({ kind: 'error', text: errText(e) });
      }
      search(queryRef.current);
    },
    [backend, search],
  );

  // ---- keyboard
  const move = useCallback(
    (delta: number) => {
      if (rows.length === 0) return;
      setSelected((s) => (s + delta + rows.length) % rows.length);
    },
    [rows.length],
  );

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.nativeEvent.isComposing) return;
    const row = rows[selected];
    switch (e.key) {
      case 'Escape':
        e.preventDefault();
        if (ai) stopAi();
        else close();
        break;
      case 'ArrowDown':
        e.preventDefault();
        if (!ai) move(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        if (!ai) move(-1);
        break;
      case 'PageDown':
        e.preventDefault();
        if (!ai && rows.length) setSelected((s) => Math.min(rows.length - 1, s + 5));
        break;
      case 'PageUp':
        e.preventDefault();
        if (!ai) setSelected((s) => Math.max(0, s - 5));
        break;
      case 'Enter':
        e.preventDefault();
        if (ai) break;
        if (query !== resultQuery) {
          // Results for the newest text are still on the way.
          if (query.trim()) pendingEnter.current = true;
          break;
        }
        if (row) void run(row, (e.shiftKey || e.ctrlKey) && (row.category === 'file' || row.category === 'folder'));
        break;
      case 'p':
      case 'P':
        if (e.ctrlKey && row && !ai) {
          e.preventDefault();
          void togglePin(row);
        }
        break;
      case 'Tab':
        e.preventDefault();
        break;
      default:
    }
  };

  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-index="${selected}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [selected, rows]);

  const sel = rows[selected];
  const q = query.trim();
  const showEmpty = !ai && q === '' && rows.length === 0;
  const noMatch = !ai && q !== '' && rows.length === 0 && resultQuery === query && !result.appsLoading;

  const hint = (() => {
    if (ai) return [['Esc', 'Back']];
    const h: string[][] = [['↑↓', 'Move']];
    if (sel?.category === 'app') h.push(['↵', 'Open'], ['Ctrl P', sel.pinned ? 'Unpin' : 'Pin']);
    else if (sel?.category === 'file' || sel?.category === 'folder') h.push(['↵', 'Open'], ['⇧↵', 'Show in folder']);
    else if (sel?.category === 'ai') h.push(['↵', 'Ask']);
    else if (sel) h.push(['↵', 'Run']);
    h.push(['Esc', 'Close']);
    return h;
  })();

  return (
    <div
      className="lx"
      data-phase={phase}
      data-motion={a?.motion ?? 'normal'}
      data-testid="launcher"
      style={{ '--accent': accent, '--on-accent': onAccent(accent) } as CSSProperties}
      onKeyDown={onKeyDown}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="lx-panel" role="dialog" aria-label="VEX Quick Launcher" data-testid="lx-panel">
        <div className="lx-search">
          <SearchIcon />
          <input
            ref={inputRef}
            data-testid="lx-input"
            role="combobox"
            aria-expanded={rows.length > 0}
            aria-controls="lx-list"
            aria-activedescendant={sel ? `lx-row-${selected}` : undefined}
            aria-label="Search apps, files and commands"
            placeholder="Search apps, files, or type a command…"
            value={query}
            autoFocus
            spellCheck={false}
            autoComplete="off"
            maxLength={600}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelected(0);
              setStatus(null);
              if (ai) setAi(null);
            }}
          />
          {q !== '' && query !== resultQuery && <Spinner />}
        </div>

        {ai ? (
          <AiPanel ai={ai} info={aiInfo} onBack={stopAi} onCopy={() => void copy(ai.answer, 'Answer')} />
        ) : (
          <div className="lx-list" id="lx-list" role="listbox" ref={listRef} data-testid="lx-list">
            {rows.map((row, i) => (
              <div key={row.key}>
                {(i === 0 || rows[i - 1].section !== row.section) && (
                  <div className="lx-section" data-testid="lx-section" role="presentation">
                    {row.section}
                  </div>
                )}
                <div
                  id={`lx-row-${i}`}
                  role="option"
                  aria-selected={i === selected}
                  aria-disabled={!row.available}
                  className="lx-row"
                  data-testid="lx-row"
                  data-index={i}
                  data-category={row.category}
                  data-selected={i === selected}
                  data-available={row.available}
                  onMouseMove={() => i !== selected && setSelected(i)}
                  onClick={() => void run(row)}
                >
                  <span className="lx-icon" data-category={row.category}>
                    {row.category === 'app' ? <AppIcon id={row.target.type === 'app' ? row.target.id : row.key} name={row.title} kind={row.appKind} /> : <RowGlyph row={row} />}
                  </span>
                  <span className="lx-text">
                    <span className="lx-title" data-calc={row.target.type === 'command' && row.target.action.type === 'calc' ? '1' : undefined}>
                      {row.title}
                    </span>
                    <span className="lx-sub">{row.available ? row.subtitle : (row.reason ?? row.subtitle)}</span>
                  </span>
                  {row.category === 'ai' && aiInfo && <span className="lx-chip" data-testid="lx-ai-chip">{aiInfo.provider} · {aiInfo.model || 'no model'}</span>}
                  {kindLabel(row) && <span className="lx-chip lx-kind">{kindLabel(row)}</span>}
                  {row.pinned && <span className="lx-pinmark" title="Pinned"><StarIcon filled /></span>}
                  {row.category === 'app' && (
                    <button
                      className="lx-act"
                      data-testid="lx-pin"
                      aria-label={row.pinned ? `Unpin ${row.title}` : `Pin ${row.title}`}
                      title={row.pinned ? 'Unpin' : 'Pin'}
                      onClick={(e) => {
                        e.stopPropagation();
                        void togglePin(row);
                      }}
                    >
                      <StarIcon filled={row.pinned} />
                    </button>
                  )}
                  {(row.category === 'file' || row.category === 'folder') && (
                    <button
                      className="lx-act lx-act-text"
                      data-testid="lx-reveal"
                      aria-label={`Show ${row.title} in its folder`}
                      onClick={(e) => {
                        e.stopPropagation();
                        void run(row, true);
                      }}
                    >
                      Show in folder
                    </button>
                  )}
                </div>
              </div>
            ))}

            {showEmpty && (
              <div className="lx-empty" data-testid="lx-empty">
                <strong>Search your PC</strong>
                <span>Start typing to find apps and files, or try a command:</span>
                <code>calc 245 * 18</code>
                <code>open downloads</code>
                <code>search cybersecurity</code>
                <span className="lx-dim">Pin apps with Ctrl+P and they will appear here.</span>
              </div>
            )}
            {result.appsLoading && q !== '' && rows.length === 0 && (
              <div className="lx-empty" data-testid="lx-loading"><Spinner /> Looking for your apps…</div>
            )}
            {noMatch && (
              <div className="lx-empty" data-testid="lx-nomatch">
                <strong>No results for “{q}”</strong>
                <span className="lx-dim">Check the spelling, or switch categories on in Settings &gt; Launcher.</span>
              </div>
            )}
          </div>
        )}

        <div className="lx-foot">
          <div className="lx-keys" data-testid="lx-keys">
            {hint.map(([k, t]) => (
              <span key={t}><kbd>{k}</kbd> {t}</span>
            ))}
          </div>
          <div className="lx-state">
            {status && (
              <span className="lx-status" data-kind={status.kind} role={status.kind === 'error' ? 'alert' : 'status'} data-testid="lx-status">
                {status.text}
              </span>
            )}
            {!status && (result.appsLoading || result.filesLoading) && (
              <span className="lx-dim" data-testid="lx-indexing"><Spinner /> {result.appsLoading ? 'Finding apps…' : 'Indexing files…'}</span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function AiPanel({ ai, info, onBack, onCopy }: { ai: AiState; info: LauncherAiInfo | null; onBack(): void; onCopy(): void }) {
  const busy = ai.state === 'sending' || ai.state === 'streaming';
  return (
    <div className="lx-ai" data-testid="lx-ai" data-state={ai.state}>
      <div className="lx-ai-head">
        <span className="lx-ai-kind">{AI_NAMES[ai.kind] ?? 'Ask'}</span>
        {info && <span className="lx-chip" data-testid="lx-ai-provider">{info.provider} · {info.model || 'no model'}</span>}
        {busy && <Spinner />}
      </div>
      <div className="lx-ai-prompt" title={ai.text}>{ai.text}</div>
      <div className="lx-ai-body" aria-live="polite" data-testid="lx-ai-answer">
        {ai.answer || (busy ? <span className="lx-dim">Waiting for the model…</span> : null)}
      </div>
      {ai.state === 'error' && (
        <div className="lx-ai-error" role="alert" data-testid="lx-ai-error">{ai.error}</div>
      )}
      {ai.state === 'cancelled' && <div className="lx-dim">Stopped.</div>}
      <div className="lx-ai-actions">
        <button className="lx-btn" onClick={onBack} data-testid="lx-ai-back">{busy ? 'Stop' : 'Back'}</button>
        {ai.answer && !busy && <button className="lx-btn" onClick={onCopy} data-testid="lx-ai-copy">Copy answer</button>}
        <span className="lx-dim lx-ai-note">Only the text you typed was sent.</span>
      </div>
    </div>
  );
}
