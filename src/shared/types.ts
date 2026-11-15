// TypeScript mirror of the Rust settings schema (crates/islet-core/src/settings.rs).
// src/shared/defaults.json is verified against Rust's Settings::default() by a cargo test.

export type Theme = 'dark' | 'light' | 'system';
export type Shape = 'pill' | 'notch';
export type AccentSource = 'artwork' | 'custom';
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
    lyricsInCollapsed: boolean;
    lyricsFont: string;
    lyricsFontSize: number;
    lyricsAnimationSpeed: number;
    karaoke: boolean;
    lyricsOffsetMs: number;
  };
  ai: {
    provider: AiProvider;
    model: string;
    systemPrompt: string;
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
