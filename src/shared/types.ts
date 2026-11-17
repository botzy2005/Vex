// TypeScript mirror of the Rust settings schema (crates/vex-core/src/settings.rs).
// src/shared/defaults.json is verified against Rust's Settings::default() by a cargo test.

export type Theme = 'dark' | 'light' | 'system';
export type Shape = 'pill' | 'notch';
export type AccentSource = 'artwork' | 'custom';
export type LyricsStyle = 'letters' | 'line';
export type Motion = 'off' | 'subtle' | 'normal' | 'bouncy';
export type AiProvider = 'ollama' | 'openai' | 'gemini' | 'claude' | 'openrouter' | 'openai-compatible';

export interface Settings {
  version: number;
  appearance: {
    theme: Theme;
    shape: Shape;
    glassOpacity: number;
    cornerRadius: number;
    accentSource: AccentSource;
    customAccent: string;
    motion: Motion;
    sizeScale: number;
  };
  island: {
    monitorIndex: number;
    offsetX: number;
    marginTop: number;
    hoverToExpand: boolean;
    hoverDelayMs: number;
    collapseDelayMs: number;
    hotkey: string;
  };
  music: {
    artwork: boolean;
    visualizer: boolean;
    lyricsEnabled: boolean;
    lyricsOnline: boolean;
    lyricsStyle: LyricsStyle;
    lyricsInCollapsed: boolean;
    lyricsFont: string;
    lyricsFontSize: number;
    lyricsAnimationSpeed: number;
    karaoke: boolean;
    lyricsOffsetMs: number;
  };
  ai: {
    provider: AiProvider;
    /** Chosen model per provider id; a provider with no entry has no model chosen. */
    models: Record<string, string>;
    systemPrompt: string;
    ollamaEndpoint: string;
    compatibleBaseUrl: string;
    contextOnRequest: boolean;
    ollamaThink: boolean;
    ollamaKeepAlive: string;
    ollamaNumCtx: number;
  };
}

export type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };
/** RFC 7386 merge patch: `null` resets a field to its default. */
export type SettingsPatch = DeepPartial<Settings>;

export interface UpdateResult {
  settings: Settings;
  warnings: string[];
}

export interface MonitorInfo {
  index: number;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  primary: boolean;
}

// ---- media (implemented by the Windows backend in Milestone 2) ----

export type PlaybackStatus = 'playing' | 'paused' | 'stopped';

export interface MediaCapabilities {
  play: boolean;
  pause: boolean;
  next: boolean;
  previous: boolean;
  seek: boolean;
}

export interface MediaSession {
  appId: string;
  title: string;
  artist: string;
  album?: string;
  /** data: or blob: URL for the artwork, if the player provides one. */
  artworkUrl?: string;
  durationMs: number | null;
  /** Playback position at `positionUpdatedAt`. */
  positionMs: number;
  /** Epoch milliseconds (Date.now()) when positionMs was sampled. */
  positionUpdatedAt: number;
  playbackRate: number;
  status: PlaybackStatus;
  capabilities: MediaCapabilities;
}

export interface MediaState {
  session: MediaSession | null;
}

export type TransportCommand =
  | { type: 'play' | 'pause' | 'toggle' | 'next' | 'previous' }
  | { type: 'seek'; positionMs: number };

// ---- assistant ----

export type AssistantState = 'idle' | 'listening' | 'thinking' | 'speaking' | 'error';

// ---- lyrics ----

export interface LyricWord {
  timeMs: number;
  text: string;
}

export interface LyricLine {
  timeMs: number;
  text: string;
  /** Present only when the source provides word-level timing (enhanced LRC). */
  words?: LyricWord[];
}

export interface LyricsDoc {
  lines: LyricLine[];
  source: string;
}

export interface LyricsQuery {
  title: string;
  artist: string;
  album?: string;
  durationMs: number | null;
}

export interface LyricsFetchResult {
  /** Raw LRC text (parse with parseLrc). */
  synced: string | null;
  plain: string | null;
  instrumental: boolean;
  source: string;
}

// ---- AI chat ----

export type ChatRole = 'user' | 'assistant';

export interface ChatMessage {
  role: ChatRole;
  content: string;
  tsMs: number;
  /** App context shared with this message (only when the user asked about it). */
  context?: string | null;
  contextLabel?: string | null;
  /** A reply that was stopped or cut off. */
  partial?: boolean;
}

export interface Conversation {
  id: string;
  title: string;
  createdMs: number;
  updatedMs: number;
  messages: ChatMessage[];
}

export interface ConversationSummary {
  id: string;
  title: string;
  updatedMs: number;
  messageCount: number;
}

export interface ContextPayload {
  label: string;
  text: string;
}

export interface SendResult {
  conversationId: string;
  requestId: number;
}

export type AiEventKind = 'delta' | 'thinking' | 'done' | 'error' | 'cancelled';

export interface AiEvent {
  requestId: number;
  conversationId: string;
  kind: AiEventKind;
  text?: string | null;
  message?: string | null;
  errorKind?: string | null;
  /** For 'thinking' events: how much hidden reasoning has arrived so far, in characters. */
  thinkingChars?: number | null;
}

export interface AiTestResult {
  ok: boolean;
  message: string;
  models: string[];
  /** Ollama only: the endpoint is on this PC. */
  local: boolean;
}

// ---- AI performance measurements (numbers only; never message text or keys)

/** What Ollama itself reports for a reply. */
export interface UsageStats {
  totalMs: number | null;
  /** Loading the model into memory (0 = it was already loaded). */
  loadMs: number | null;
  promptTokens: number | null;
  promptEvalMs: number | null;
  /** Includes hidden thinking tokens. */
  generatedTokens: number | null;
  generationMs: number | null;
  tokensPerSec: number | null;
  doneReason: string | null;
}

export interface RequestTiming {
  headersMs: number | null;
  firstThinkingMs: number | null;
  firstTokenMs: number | null;
  totalMs: number;
  thinkingChunks: number;
  textChunks: number;
  thinkingChars: number;
  textChars: number;
}

export interface RequestMetrics {
  seq: number;
  startedAtMs: number;
  provider: string;
  model: string;
  outcome: 'done' | 'cancelled' | 'error';
  errorKind: string | null;
  payloadBytes: number;
  messagesSent: number;
  contextChars: number;
  historyTrimmed: boolean;
  think: boolean | null;
  numCtx: number | null;
  keepAlive: string | null;
  prepareMs: number;
  timing: RequestTiming;
  usage: UsageStats | null;
}

export interface MetricsView {
  requests: RequestMetrics[];
  requestsSent: number;
}

export interface BenchRow {
  model: string;
  think: boolean;
  mode: 'preload' | 'direct' | 'vex';
  ok: boolean;
  error: string | null;
  wallMs: number;
  headersMs: number | null;
  firstThinkingMs: number | null;
  firstTokenMs: number | null;
  visibleChars: number;
  thinkingChars: number;
  payloadBytes: number;
  usage: UsageStats | null;
}
