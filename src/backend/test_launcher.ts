/**
 * TEST HARNESS — NOT PART OF THE SHIPPED APP.
 *
 * Browser stand-in for the Quick Launcher's native side. The real ranking, command parsing and safety
 * checks live in Rust (`vex_core::launcher`, unit-tested there); this keeps just enough behaviour for
 * the UI tests: fixture apps and files, the result sections, the explicit web and AI rows, pins,
 * recents, and a recorded log of every run the page asked for.
 */
import type {
  AiEvent,
  LauncherAction,
  LauncherAiInfo,
  LauncherAiKind,
  LauncherCategory,
  LauncherFileKind,
  LauncherPin,
  LauncherRow,
  LauncherRunOutcome,
  LauncherSearchResult,
  LauncherTarget,
  LauncherView,
  Settings,
} from '../shared/types';
import type { Backend, Unsub } from './backend';

export interface FixtureApp {
  id: string;
  name: string;
  kind: 'desktop' | 'store';
  /** A data URL, or omitted for the lettered-tile fallback. */
  icon?: string | null;
}

export interface FixtureFile {
  path: string;
  name: string;
  kind: LauncherFileKind;
}

export interface LauncherHarness {
  setApps(apps: FixtureApp[]): void;
  setFiles(files: FixtureFile[]): void;
  /** Pretend the first app discovery / file index build has not finished (or has). */
  setLoading(o: { apps?: boolean; files?: boolean }): void;
  /** Delay every search by this many ms (to test rapid typing and stale answers). */
  setSearchDelay(ms: number): void;
  /** Make the next run fail with this message. */
  failNextRun(message: string | null): void;
  setAiInfo(info: LauncherAiInfo): void;
  setAiScript(s: { chunks?: string[]; error?: string; delayMs?: number }): void;
  /** Simulate the global shortcut: show the window / ask it to close. */
  emitOpen(): void;
  emitClose(): void;
  emitIndex(): void;
  readonly runs: Array<{ input: string; target: LauncherTarget; reveal: boolean }>;
  readonly searches: string[];
  readonly hides: number;
  readonly aiSends: Array<{ kind: LauncherAiKind; text: string; language: string | null }>;
  readonly iconRequests: string[];
  reset(): void;
}

const STORE = 'vex-test-launcher';

const DEFAULT_APPS: FixtureApp[] = [
  { id: 'C:\\Start\\Visual Studio Code.lnk', name: 'Visual Studio Code', kind: 'desktop' },
  { id: 'C:\\Start\\Notepad.lnk', name: 'Notepad', kind: 'desktop' },
  { id: 'C:\\Start\\Google Chrome.lnk', name: 'Google Chrome', kind: 'desktop' },
  { id: 'shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App', name: 'Calculator', kind: 'store' },
  { id: 'shell:AppsFolder\\Microsoft.Paint_8wekyb3d8bbwe!App', name: 'Paint', kind: 'store' },
];

const DEFAULT_FILES: FixtureFile[] = [
  { path: 'C:\\Users\\me\\Documents\\budget 2026.xlsx', name: 'budget 2026.xlsx', kind: 'document' },
  { path: 'C:\\Users\\me\\Documents\\Projects', name: 'Projects', kind: 'folder' },
  { path: 'C:\\Users\\me\\Pictures\\holiday.png', name: 'holiday.png', kind: 'image' },
  { path: 'C:\\Users\\me\\Downloads\\setup.exe', name: 'setup.exe', kind: 'other' },
];

interface Saved {
  pins: LauncherPin[];
  recents: Array<LauncherPin & { at: number }>;
  history: string[];
}

function load(): Saved {
  try {
    const s = localStorage.getItem(STORE);
    if (s) return { pins: [], recents: [], history: [], ...JSON.parse(s) };
  } catch {
    /* ignore */
  }
  return { pins: [], recents: [], history: [] };
}

/** A tiny recursive-descent calculator (no eval, no Function). */
function calc(src: string): number | null {
  const s = src.replace(/[×x]/g, '*').replace(/÷/g, '/');
  if (!/^[\d\s+\-*/%^().]+$/.test(s)) return null;
  let i = 0;
  const ws = () => {
    while (s[i] === ' ') i++;
  };
  const num = (): number => {
    ws();
    if (s[i] === '(') {
      i++;
      const v = expr();
      ws();
      if (s[i] !== ')') throw new Error('paren');
      i++;
      return v;
    }
    if (s[i] === '-') {
      i++;
      return -num();
    }
    const m = /^\d+(\.\d+)?/.exec(s.slice(i));
    if (!m) throw new Error('num');
    i += m[0].length;
    return parseFloat(m[0]);
  };
  const pow = (): number => {
    const b = num();
    ws();
    if (s[i] === '^') {
      i++;
      return Math.pow(b, pow());
    }
    return b;
  };
  const term = (): number => {
    let v = pow();
    for (;;) {
      ws();
      const c = s[i];
      if (c !== '*' && c !== '/' && c !== '%') return v;
      i++;
      const r = pow();
      v = c === '*' ? v * r : c === '/' ? v / r : v % r;
    }
  };
  const expr = (): number => {
    let v = term();
    for (;;) {
      ws();
      const c = s[i];
      if (c !== '+' && c !== '-') return v;
      i++;
      const r = term();
      v = c === '+' ? v + r : v - r;
    }
  };
  try {
    const v = expr();
    ws();
    return i === s.length && Number.isFinite(v) ? v : null;
  } catch {
    return null;
  }
}

function score(query: string, name: string): number | null {
  const q = query.toLowerCase();
  const n = name.toLowerCase();
  if (n === q) return 1000;
  if (n.startsWith(q)) return 800;
  if (n.split(/[^a-z0-9]+/).some((w) => w.startsWith(q))) return 600;
  if (n.includes(q)) return 400;
  if (q.length >= 2) {
    let qi = 0;
    for (const c of n) if (c === q[qi]) qi++;
    if (qi === q.length) return 200;
  }
  return null;
}

export function createTestLauncher(getSettings: () => Settings): { api: Partial<Backend>; harness: LauncherHarness } {
  let apps = [...DEFAULT_APPS];
  let files = [...DEFAULT_FILES];
  let loading = { apps: false, files: false };
  let delay = 0;
  let failRun: string | null = null;
  let aiInfo: LauncherAiInfo = { provider: 'Ollama', model: 'qwen3.5:2b', ready: true, problem: null };
  let aiScript: { chunks?: string[]; error?: string; delayMs?: number } = { chunks: ['A ', 'short ', 'answer.'], delayMs: 20 };
  let nextRequest = 1000;
  const runs: LauncherHarness['runs'] = [];
  const searches: string[] = [];
  const aiSends: LauncherHarness['aiSends'] = [];
  const iconRequests: string[] = [];
  let hides = 0;
  const aiSubs = new Set<(e: AiEvent) => void>();
  const openSubs = new Set<() => void>();
  const closeSubs = new Set<() => void>();
  const indexSubs = new Set<() => void>();

  const saved = (): Saved => load();
  const persist = (s: Saved) => {
    try {
      localStorage.setItem(STORE, JSON.stringify(s));
    } catch {
      /* ignore */
    }
  };
  const sub = <T,>(set: Set<(v: T) => void>, cb: (v: T) => void): Promise<Unsub> => {
    set.add(cb);
    return Promise.resolve(() => void set.delete(cb));
  };

  const appRow = (section: LauncherRow['section'], a: { id: string; name: string; kind: 'desktop' | 'store' }, pinned: boolean, installed = true): LauncherRow => ({
    key: `app:${a.id}`,
    section,
    category: 'app',
    title: a.name,
    subtitle: installed ? (a.kind === 'store' ? 'Microsoft Store app' : 'Application') : 'No longer installed',
    target: { type: 'app', id: a.id },
    available: installed,
    reason: installed ? null : 'This app is no longer installed. Remove it from the list.',
    pinned,
    appKind: a.kind,
    fileKind: null,
  });
  const cmdRow = (id: string, title: string, subtitle: string, action: LauncherAction, category: LauncherCategory = 'command', section: LauncherRow['section'] = 'Commands', available = true, reason: string | null = null): LauncherRow => ({
    key: `cmd:${id}:${title}`,
    section,
    category,
    title,
    subtitle,
    target: { type: 'command', action },
    available,
    reason,
    pinned: false,
    appKind: null,
    fileKind: null,
  });

  const search = (raw: string): LauncherRow[] => {
    const q = raw.replace(/[\u0000-\u001f]/g, '').trim();
    const L = getSettings().launcher;
    const st = saved();
    const max = Math.min(12, Math.max(3, L.maxResults));
    const pinnedIds = new Set(st.pins.map((p) => p.id));
    if (!q) {
      if (!L.apps) return [];
      const have = (id: string) => (loading.apps ? true : apps.some((a) => a.id === id));
      const rows = st.pins.map((p) => appRow('Pinned', p, true, have(p.id)));
      if (L.rememberRecents) {
        for (const r of st.recents.filter((r) => !pinnedIds.has(r.id)).slice(0, 6)) rows.push(appRow('Recent', r, false, have(r.id)));
      }
      return rows;
    }

    const cmds: LauncherRow[] = [];
    const [word, ...restWords] = q.split(/\s+/);
    const rest = restWords.join(' ');
    const w = word.toLowerCase();
    if (L.commands) {
      const expr = w === 'calc' || w === 'calculate' ? rest : w.startsWith('=') ? q.slice(1) : /^[\d(.\-+]/.test(q) && /[+\-*/%^×÷(]/.test(q) ? q : null;
      if (expr !== null) {
        const v = calc(expr);
        if (v === null) cmds.push(cmdRow('calc', 'Calculator', expr, { type: 'invalid' }, 'command', 'Commands', false, 'I could not read that expression.'));
        else {
          const result = String(Math.round(v * 1e10) / 1e10);
          cmds.push(cmdRow('calc', result, `${expr.trim()} =`, { type: 'calc', expression: expr.trim(), result }));
        }
      }
      if (w === 'time' && !rest) cmds.push(cmdRow('time', 'Current time', 'Copy to clipboard', { type: 'time' }));
      if (w === 'date' && !rest) cmds.push(cmdRow('date', "Today's date", 'Copy to clipboard', { type: 'date' }));
      if (w === 'open') {
        for (const f of ['downloads', 'documents', 'desktop', 'pictures', 'videos', 'music'] as const) {
          if (!rest || f.startsWith(rest.toLowerCase())) cmds.push(cmdRow('open', `Open ${f}`, 'Folder', { type: 'openFolder', folder: f }));
        }
        if (!rest || 'settings'.startsWith(rest.toLowerCase())) cmds.push(cmdRow('open', 'Open VEX settings', 'VEX', { type: 'openVexSettings' }));
      }
      if (w === 'timer') {
        cmds.push(cmdRow('timer', `Timer: ${rest || '?'}`, 'Countdown timer', { type: 'timer', seconds: 600, label: '' }, 'command', 'Commands', false, 'Timers arrive with the Widgets Hub and are not available yet.'));
      }
      if (w === 'note') {
        cmds.push(cmdRow('note', 'Save note', rest, { type: 'note', text: rest }, 'command', 'Commands', false, 'Notes arrive with the Widgets Hub and are not available yet.'));
      }
    }
    const explicitWeb = L.web && (w === 'search' || w === 'web' || w === 'google') && rest;
    if (explicitWeb) cmds.push(cmdRow('search', `Search the web for \u201c${rest}\u201d`, 'Opens your default browser', { type: 'webSearch', query: rest }, 'web', 'Web'));
    const aiKinds: Record<string, LauncherAiKind> = { ask: 'ask', ai: 'ask', explain: 'explain', summarize: 'summarize', summarise: 'summarize', tldr: 'summarize', rewrite: 'rewrite', translate: 'translate' };
    const explicitAi = L.ai && aiKinds[w] && rest;
    if (explicitAi) {
      const kind = aiKinds[w];
      let text = rest;
      let language: string | null = null;
      if (kind === 'translate') {
        const m = /^(.*)\s(?:to|into)\s([A-Za-z -]{1,24})$/i.exec(rest);
        if (m) {
          text = m[1];
          language = m[2];
        }
      }
      cmds.push(cmdRow(kind, kind === 'ask' ? 'Ask AI' : `${kind[0].toUpperCase()}${kind.slice(1)} with AI`, text, { type: 'ai', kind, text, language }, 'ai', 'AI'));
    }

    const first = q.charAt(0);
    const commandsFirst = cmds.some((c) => c.category === 'command') && (/\s/.test(q) || first === '=' || /[\d(\-.+]/.test(first));

    const appRows: LauncherRow[] = [];
    if (L.apps) {
      const found = apps
        .map((a) => ({ a, s: score(q, a.name) }))
        .filter((x): x is { a: FixtureApp; s: number } => x.s !== null)
        .map((x) => ({ ...x, s: x.s + (pinnedIds.has(x.a.id) ? 150 : 0) }))
        .sort((x, y) => y.s - x.s || x.a.name.localeCompare(y.a.name))
        .slice(0, max);
      for (const { a } of found) appRows.push(appRow('Applications', a, pinnedIds.has(a.id)));
    }
    const fileRows: LauncherRow[] = [];
    if (L.files && q.length >= 2) {
      const found = files
        .map((f) => ({ f, s: score(q, f.name) }))
        .filter((x): x is { f: FixtureFile; s: number } => x.s !== null)
        .sort((x, y) => y.s - x.s)
        .slice(0, Math.max(3, Math.floor(max / 2)));
      for (const { f } of found) {
        const folder = f.kind === 'folder';
        const cut = f.path.lastIndexOf('\\');
        fileRows.push({
          key: `file:${f.path}`,
          section: folder ? 'Folders' : 'Files',
          category: folder ? 'folder' : 'file',
          title: f.name,
          subtitle: f.path.slice(0, cut),
          target: { type: 'file', path: f.path },
          available: true,
          reason: null,
          pinned: false,
          appKind: null,
          fileKind: f.kind,
        });
      }
    }
    const plain = cmds.filter((c) => c.category === 'command');
    let main = commandsFirst ? [...plain, ...appRows, ...fileRows] : [...appRows, ...plain, ...fileRows];
    main = main.slice(0, max);
    const tail = cmds.filter((c) => c.category !== 'command');
    if (L.web && !explicitWeb) tail.push(cmdRow('web', `Search the web for \u201c${q}\u201d`, 'Opens your default browser', { type: 'webSearch', query: q }, 'web', 'Web'));
    if (L.ai && !explicitAi && (q.endsWith('?') || q.split(/\s+/).length >= 3)) {
      tail.push(cmdRow('ask', 'Ask AI', q, { type: 'ai', kind: 'ask', text: q, language: null }, 'ai', 'AI'));
    }
    return [...main, ...tail];
  };

  const view = (): LauncherView => {
    const st = saved();
    return {
      pins: st.pins,
      recents: st.recents,
      history: st.history,
      appsCount: apps.length,
      appsUpdated: Math.floor(Date.now() / 1000),
      filesCount: files.length,
      filesUpdated: Math.floor(Date.now() / 1000),
      filesTruncated: false,
      filesUnreadable: 0,
      refreshing: false,
    };
  };

  const api: Partial<Backend> = {
    async launcherSearch(query): Promise<LauncherSearchResult> {
      searches.push(query);
      if (delay) await new Promise((r) => setTimeout(r, delay));
      const L = getSettings().launcher;
      return { rows: search(query), appsLoading: L.apps && loading.apps, filesLoading: L.files && loading.files };
    },
    async launcherRun(input, target, reveal): Promise<LauncherRunOutcome> {
      runs.push({ input, target, reveal: !!reveal });
      if (failRun) {
        const m = failRun;
        failRun = null;
        throw new Error(m);
      }
      if (target.type === 'app') {
        const a = apps.find((x) => x.id === target.id);
        if (!a) throw new Error('That app is no longer installed.');
        if (getSettings().launcher.rememberRecents) {
          const st = saved();
          st.recents = [{ id: a.id, name: a.name, kind: a.kind, at: Date.now() }, ...st.recents.filter((r) => r.id !== a.id)].slice(0, 20);
          persist(st);
        }
      }
      if (target.type === 'file' && /\.exe$/i.test(target.path)) return { message: 'Programs and scripts are shown in Explorer, not run from here.' };
      return { message: null };
    },
    launcherHide: async () => void hides++,
    launcherState: async () => view(),
    async launcherSetPin(id, pinned) {
      const st = saved();
      if (pinned) {
        const a = apps.find((x) => x.id === id);
        if (!a) throw new Error("That app is not in the launcher's list.");
        if (st.pins.length >= 30) throw new Error('You can pin up to 30 apps. Unpin one first.');
        if (!st.pins.some((p) => p.id === id)) st.pins.push({ id: a.id, name: a.name, kind: a.kind });
      } else st.pins = st.pins.filter((p) => p.id !== id);
      persist(st);
    },
    async launcherRecordSearch(text) {
      if (!getSettings().launcher.rememberRecents) return;
      const st = saved();
      st.history = [text, ...st.history.filter((h) => h !== text)].slice(0, 30);
      persist(st);
    },
    async launcherClearHistory() {
      const st = saved();
      st.recents = [];
      st.history = [];
      persist(st);
    },
    launcherRefresh: async () => indexSubs.forEach((cb) => cb()),
    async launcherAppIcon(id) {
      iconRequests.push(id);
      return apps.find((a) => a.id === id)?.icon ?? null;
    },
    launcherAiInfo: async () => aiInfo,
    async launcherAiSend(kind, text, language) {
      aiSends.push({ kind, text, language });
      if (!getSettings().launcher.ai) throw new Error('AI commands are switched off in Launcher settings.');
      if (!aiInfo.ready) throw new Error(aiInfo.problem ?? 'AI is not set up.');
      const requestId = nextRequest++;
      const emit = (e: Partial<AiEvent> & Pick<AiEvent, 'kind'>) => aiSubs.forEach((cb) => cb({ requestId, conversationId: 'launcher', ...e }));
      void (async () => {
        for (const c of aiScript.chunks ?? []) {
          await new Promise((r) => setTimeout(r, aiScript.delayMs ?? 20));
          emit({ kind: 'delta', text: c });
        }
        await new Promise((r) => setTimeout(r, aiScript.delayMs ?? 20));
        if (aiScript.error) emit({ kind: 'error', message: aiScript.error, errorKind: 'provider' });
        else emit({ kind: 'done' });
      })();
      return requestId;
    },
    onLauncherAi: (cb) => sub(aiSubs, cb),
    onLauncherOpen: (cb) => sub(openSubs, cb),
    onLauncherClose: (cb) => sub(closeSubs, cb),
    onLauncherIndex: (cb) => sub(indexSubs, cb),
  };

  const harness: LauncherHarness = {
    setApps: (a) => void (apps = a),
    setFiles: (f) => void (files = f),
    setLoading: (o) => void (loading = { ...loading, ...o }),
    setSearchDelay: (ms) => void (delay = ms),
    failNextRun: (m) => void (failRun = m),
    setAiInfo: (i) => void (aiInfo = i),
    setAiScript: (s) => void (aiScript = s),
    emitOpen: () => openSubs.forEach((cb) => cb()),
    emitClose: () => closeSubs.forEach((cb) => cb()),
    emitIndex: () => indexSubs.forEach((cb) => cb()),
    runs,
    searches,
    get hides() {
      return hides;
    },
    aiSends,
    iconRequests,
    reset() {
      apps = [...DEFAULT_APPS];
      files = [...DEFAULT_FILES];
      loading = { apps: false, files: false };
      delay = 0;
      failRun = null;
      aiInfo = { provider: 'Ollama', model: 'qwen3.5:2b', ready: true, problem: null };
      aiScript = { chunks: ['A ', 'short ', 'answer.'], delayMs: 20 };
      runs.length = 0;
      searches.length = 0;
      aiSends.length = 0;
      iconRequests.length = 0;
      hides = 0;
      try {
        localStorage.removeItem(STORE);
      } catch {
        /* ignore */
      }
    },
  };
  return { api, harness };
}
