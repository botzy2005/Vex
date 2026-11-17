import type {
  AiEvent,
  AiTestResult,
  AssistantState,
  BenchRow,
  MetricsView,
  ContextPayload,
  Conversation,
  ConversationSummary,
  SendResult,
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

  /**
   * Send a chat message. Resolves once the request has started (the answer then streams
   * through `onAiEvent`); rejects with a readable reason if nothing could be started
   * (no model chosen, no API key, still answering…). In that case nothing is stored.
   */
  aiSend(args: { conversationId: string | null; text: string; context: ContextPayload | null }): Promise<SendResult>;
  /** Stop the answer that is currently streaming. */
  aiCancel(): Promise<void>;
  onAiEvent(cb: (e: AiEvent) => void): Promise<Unsub>;

  /** Whether an API key is saved for this provider. The key itself is never returned. */
  aiKeyStatus(provider: string): Promise<boolean>;
  /** Save a key to the OS credential store (Windows Credential Manager). */
  aiSetKey(provider: string, key: string): Promise<void>;
  aiClearKey(provider: string): Promise<void>;
  /** Models the provider (or the local Ollama install) offers. Rejects with a readable reason. */
  aiListModels(provider: string): Promise<string[]>;
  /** Check the connection / key. Never rejects for an expected failure: `ok` is false with a message. */
  aiTest(provider: string): Promise<AiTestResult>;

  /** Timing of the most recent chat requests (numbers only). */
  aiMetrics(): Promise<MetricsView>;
  aiMetricsClear(): Promise<void>;
  /** Load an installed Ollama model into memory now (never downloads). */
  aiPreload(model: string): Promise<BenchRow>;
  /** Run the same short prompt direct-to-Ollama and through VEX for each model. Rows also arrive via onBenchRow. */
  aiBenchmark(models: string[], includeThinking: boolean): Promise<BenchRow[]>;
  onBenchRow(cb: (row: BenchRow) => void): Promise<Unsub>;

  chatList(): Promise<ConversationSummary[]>;
  chatGet(id: string): Promise<Conversation | null>;
  chatDelete(id: string): Promise<void>;
  chatClearAll(): Promise<void>;

  /** Show an assistant state on the island (used by Settings to preview the face). */
  assistantPreview(state: AssistantState): Promise<void>;
  onAssistantState(cb: (s: AssistantState) => void): Promise<Unsub>;
}
