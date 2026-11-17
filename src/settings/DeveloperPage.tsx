import { useCallback, useEffect, useRef, useState } from 'react';
import { useBackend, useSubscription } from '../backend/hooks';
import type { BenchRow, MetricsView, RequestMetrics, Settings } from '../shared/types';
import { Row, Toggle } from './controls';

const sec = (ms: number | null | undefined) => (ms == null ? '–' : ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(ms < 10_000 ? 2 : 1)} s`);
const num = (n: number | null | undefined, digits = 0) => (n == null ? '–' : n.toFixed(digits));
const kb = (b: number) => (b < 1024 ? `${b} B` : `${(b / 1024).toFixed(1)} kB`);

const MODE_LABEL: Record<BenchRow['mode'], string> = {
  preload: 'Load model',
  direct: 'Direct to Ollama',
  vex: 'Through VEX',
};

export function DeveloperPage({ s }: { s: Settings }) {
  const backend = useBackend();
  const [view, setView] = useState<MetricsView>({ requests: [], requestsSent: 0 });
  const refresh = useCallback(() => void backend.aiMetrics().then(setView).catch(() => {}), [backend]);

  useEffect(() => {
    refresh();
    const id = window.setInterval(refresh, 2000);
    return () => window.clearInterval(id);
  }, [refresh]);

  return (
    <section data-testid="dev-page">
      <h2>Developer</h2>
      <p className="dim">
        Timing for each AI request. Only numbers and the model name are recorded, never your messages, music context or keys, and nothing is saved to disk: it resets when VEX restarts.
      </p>
      <Measurements view={view} onClear={() => void backend.aiMetricsClear().then(refresh)} />
      {s.ai.provider === 'ollama' ? <Benchmark s={s} /> : <p className="dim" data-testid="bench-needs-ollama">The Ollama benchmark appears here when the provider is Ollama.</p>}
    </section>
  );
}

function Measurements({ view, onClear }: { view: MetricsView; onClear(): void }) {
  return (
    <div data-testid="metrics">
      <div className="dev-head">
        <h3>Recent requests</h3>
        <span className="dim" data-testid="requests-sent">{view.requestsSent} sent since VEX started</span>
        <button type="button" className="btn" data-testid="metrics-clear" disabled={view.requests.length === 0} onClick={onClear}>Clear</button>
      </div>
      {view.requests.length === 0 ? (
        <div className="empty-note" data-testid="metrics-empty">No requests yet. Send a message in the assistant, then come back.</div>
      ) : (
        <div className="table-wrap">
          <table className="dev-table">
            <thead>
              <tr>
                <th>#</th><th>Model</th><th>Result</th><th title="Time until the first visible word">First word</th><th title="Hidden reasoning before the answer">Thinking</th>
                <th>Total</th><th title="Ollama's model-load time (0 = was already loaded)">Load</th><th>Tokens</th><th>Tok/s</th>
                <th title="Size of the request body">Sent</th><th title="Messages in the request / whether older ones were left out">Msgs</th><th title="Characters in the prompt, about 4 per token">Prompt</th><th>Settings</th>
              </tr>
            </thead>
            <tbody>
              {view.requests.map((r) => <MetricRow key={r.seq} r={r} />)}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function MetricRow({ r }: { r: RequestMetrics }) {
  const u = r.usage;
  const t = r.timing;
  return (
    <tr data-testid="metric-row" data-outcome={r.outcome}>
      <td>{r.seq}</td>
      <td title={r.provider}>{r.model}</td>
      <td className={r.outcome === 'done' ? 'good' : 'bad'}>{r.outcome === 'error' ? `error${r.errorKind ? ` (${r.errorKind})` : ''}` : r.outcome}</td>
      <td data-testid="m-first">{sec(t.firstTokenMs)}</td>
      <td data-testid="m-thinking">{t.thinkingChars > 0 ? `${sec(t.firstThinkingMs !== null && t.firstTokenMs !== null ? t.firstTokenMs - t.firstThinkingMs : null)} · ${t.thinkingChars} ch` : '–'}</td>
      <td>{sec(t.totalMs)}</td>
      <td>{sec(u?.loadMs)}</td>
      <td>{num(u?.generatedTokens)}</td>
      <td data-testid="m-tps">{num(u?.tokensPerSec, 1)}</td>
      <td>{kb(r.payloadBytes)}</td>
      <td>{r.messagesSent}{r.historyTrimmed ? ' (trimmed)' : ''}</td>
      <td>{r.contextChars.toLocaleString()} ch</td>
      <td>{r.provider === 'ollama' ? `think ${r.think ? 'on' : 'off'} · ctx ${r.numCtx ?? '–'} · keep ${r.keepAlive ?? '–'}` : '–'}</td>
    </tr>
  );
}

function Benchmark({ s }: { s: Settings }) {
  const backend = useBackend();
  const [models, setModels] = useState<string[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [withThinking, setWithThinking] = useState(false);
  const [rows, setRows] = useState<BenchRow[]>([]);
  const [running, setRunning] = useState(false);
  const alive = useRef(true);
  useEffect(() => () => void (alive.current = false), []);

  useEffect(() => {
    let dead = false;
    backend
      .aiListModels('ollama')
      .then((m) => !dead && (setModels(m), setErr(null)))
      .catch((e) => !dead && (setModels([]), setErr(e instanceof Error ? e.message : String(e))));
    return () => {
      dead = true;
    };
  }, [backend, s.ai.ollamaEndpoint]);

  useSubscription<BenchRow>((cb) => backend.onBenchRow(cb), (r) => running && setRows((cur) => [...cur, r]));

  const run = async () => {
    setRows([]);
    setRunning(true);
    try {
      await backend.aiBenchmark([...picked], withThinking);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      if (alive.current) setRunning(false);
    }
  };

  const toggle = (m: string) =>
    setPicked((cur) => {
      const n = new Set(cur);
      if (n.has(m)) n.delete(m);
      else n.add(m);
      return n;
    });

  return (
    <div data-testid="benchmark">
      <h3>Ollama speed test</h3>
      <p className="dim">
        Sends “Say hello in one sentence.” to each model you tick, once straight to Ollama and once through VEX, with the same settings, so you can see whether any slowness is in VEX or in the model itself.
        It uses your already-installed models only and never downloads anything. A large model can take a few minutes the first time.
      </p>
      {models === null && <div className="empty-note">Looking for installed models…</div>}
      {err && <div className="status-line bad" role="alert" data-testid="bench-error">{err}</div>}
      {models !== null && models.length === 0 && !err && <div className="empty-note" data-testid="bench-no-models">Ollama has no installed models.</div>}
      {models && models.length > 0 && (
        <>
          <div className="bench-models" data-testid="bench-models">
            {models.map((m) => (
              <label key={m}>
                <input type="checkbox" checked={picked.has(m)} disabled={running} data-testid="bench-model" onChange={() => toggle(m)} /> {m}
              </label>
            ))}
          </div>
          <Row label="Also test with thinking on" hint="Reasoning models can spend a long time on hidden thinking before answering. Capped at 128 tokens so the test cannot run away.">
            <Toggle label="Also test with thinking on" testId="bench-think" checked={withThinking} onChange={setWithThinking} />
          </Row>
          <button type="button" className="btn" data-testid="bench-run" disabled={running || picked.size === 0} onClick={() => void run()}>
            {running ? 'Running…' : 'Run speed test'}
          </button>
        </>
      )}
      {rows.length > 0 && (
        <div className="table-wrap">
          <table className="dev-table" data-testid="bench-table">
            <thead>
              <tr><th>Model</th><th>Step</th><th>Thinking</th><th>Result</th><th>First word</th><th>Total</th><th>Load</th><th>Tokens</th><th>Tok/s</th><th>Sent</th></tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={i} data-testid="bench-row" data-mode={r.mode} data-ok={r.ok}>
                  <td>{r.model}</td>
                  <td>{MODE_LABEL[r.mode]}</td>
                  <td>{r.mode === 'preload' ? '–' : r.think ? 'on' : 'off'}</td>
                  <td className={r.ok ? 'good' : 'bad'} title={r.error ?? ''}>{r.ok ? 'ok' : r.error ?? 'failed'}</td>
                  <td>{r.mode === 'preload' ? '–' : sec(r.firstTokenMs ?? r.firstThinkingMs)}</td>
                  <td>{sec(r.wallMs)}</td>
                  <td>{sec(r.usage?.loadMs)}</td>
                  <td>{num(r.usage?.generatedTokens)}</td>
                  <td>{num(r.usage?.tokensPerSec, 1)}</td>
                  <td>{r.payloadBytes ? kb(r.payloadBytes) : '–'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
