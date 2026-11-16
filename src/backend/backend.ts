import type {
  AssistantState,
  LyricsFetchResult,
  LyricsQuery,
  MediaState,
  MonitorInfo,
  Settings,
  SettingsPatch,
  TransportCommand,
  UpdateResult,
} from '../shared/types';

export type Unsub = () => void;

/**
 * Everything the UI needs from the native side. The Tauri implementation maps
 * these onto Rust commands/events; `test.ts` provides an in-browser stand-in
 * used only by the dev server and the automated UI tests.
 */
export interface Backend {
  kind: 'tauri' | 'test';

  getSettings(): Promise<Settings>;
  updateSettings(patch: SettingsPatch): Promise<UpdateResult>;
  onSettingsChanged(cb: (s: Settings) => void): Promise<Unsub>;

  listMonitors(): Promise<MonitorInfo[]>;
  /** Ask the OS window to be this many logical pixels (it stays top-centred). */
  setIslandSize(width: number, height: number): Promise<void>;
  openSettings(): Promise<void>;
  quit(): Promise<void>;
  onIslandToggle(cb: () => void): Promise<Unsub>;

  /** Milestone 2 provides a real Windows media session; until then `session` is null. */
  getMedia(): Promise<MediaState>;
  onMedia(cb: (s: MediaState) => void): Promise<Unsub>;
  /** Rejects when the command is unsupported or no backend exists. */
  mediaTransport(cmd: TransportCommand): Promise<void>;

  /**
   * Automatic synced-lyrics lookup (LRCLIB, done natively). Resolves null when
   * the track has none or lookup is switched off; rejects on network failure.
   */
  lyricsFetch(track: LyricsQuery): Promise<LyricsFetchResult | null>;

  /** Rejects with a human-readable reason when no AI provider is connected (Milestone 5). */
  assistantSend(text: string): Promise<void>;
  /** Show an assistant state on the island (used by Settings to preview the face). */
  assistantPreview(state: AssistantState): Promise<void>;
  onAssistantState(cb: (s: AssistantState) => void): Promise<Unsub>;
}
