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
  NotificationsView,
  ClipboardItem,
  ClipboardStatus,
  CaptureMode,
  ShotSource,
  ShotView,
  Settings,
  SettingsPatch,
  LauncherAiInfo,
  LauncherAiKind,
  LauncherRunOutcome,
  LauncherSearchResult,
  LauncherTarget,
  LauncherView,
  HubLayout,
  HubStatus,
  HubStatusEntry,
  HubTimer,
  TimerFinished,
  WidgetDef,
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
  /**
   * Move the island along the top edge (logical px from the screen centre). `commit: false` only moves it
   * (used while dragging); `commit: true` moves it and saves the position. Resolves with the offset that
   * was applied after keeping the island inside its monitor.
   */
  setIslandOffset(offsetX: number, commit: boolean): Promise<number>;
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

  /** Windows notifications (local only; never sent anywhere). Null until the first report. */
  getNotifications(): Promise<NotificationsView | null>;
  onNotifications(cb: (v: NotificationsView) => void): Promise<Unsub>;
  /** Hide from the island (mark read); stays in the history. */
  notificationDismiss(id: number): Promise<void>;
  notificationDismissAll(): Promise<void>;
  /** Remove one notification from the history. */
  notificationClear(id: number): Promise<void>;
  notificationClearAll(): Promise<void>;
  /** Open the app that posted it. Rejects with a readable reason when Windows cannot. */
  notificationOpen(id: number): Promise<void>;

  /**
   * Local clipboard history (never sent anywhere, never given to the assistant). The change event
   * carries only the status; ask for the list when it is shown.
   */
  clipboardStatus(): Promise<ClipboardStatus>;
  onClipboard(cb: (s: ClipboardStatus) => void): Promise<Unsub>;
  /** Pinned first, then newest first. A query filters text entries. */
  clipboardList(query: string): Promise<ClipboardItem[]>;
  /** Put an entry back on the clipboard. Rejects with a readable reason. */
  clipboardCopy(id: number): Promise<void>;
  clipboardPin(id: number, pinned: boolean): Promise<void>;
  clipboardDelete(id: number): Promise<void>;
  /** Remove entries; `keepPinned` leaves the pinned ones. */
  clipboardClear(keepPinned: boolean): Promise<void>;

  /**
   * Screenshots. Pixels are captured natively and stay on this PC. `region` opens the selection
   * overlay and resolves; the capture then arrives through `onScreenshotCaptured`.
   */
  screenshotCapture(mode: CaptureMode): Promise<void>;
  /** From the selection overlay: a drag in page pixels, measured from that overlay's top-left corner. */
  screenshotRegionSelect(monitor: number, x0: number, y0: number, x1: number, y1: number): Promise<void>;
  screenshotRegionCancel(): Promise<void>;
  /** Newest first. */
  screenshotList(): Promise<ShotView[]>;
  /** The full-size PNG as a data URL. */
  screenshotImage(id: number): Promise<string>;
  screenshotDelete(id: number): Promise<void>;
  screenshotClear(): Promise<void>;
  screenshotCopy(source: ShotSource): Promise<void>;
  /** Save with the default format and folder, or ask where. Resolves the saved path, or null if cancelled. */
  screenshotSave(source: ShotSource, ask: boolean): Promise<string | null>;
  screenshotPickFolder(): Promise<string | null>;
  screenshotReveal(path: string): Promise<void>;
  screenshotOpenEditor(id: number): Promise<void>;
  onScreenshotCaptured(cb: (s: ShotView) => void): Promise<Unsub>;
  onScreenshotError(cb: (message: string) => void): Promise<Unsub>;
  /** The editor window was asked to open a different capture. */
  onEditorLoad(cb: (id: number) => void): Promise<Unsub>;

  /** Show an assistant state on the island (used by Settings to preview the face). */
  assistantPreview(state: AssistantState): Promise<void>;
  onAssistantState(cb: (s: AssistantState) => void): Promise<Unsub>;

  /**
   * Quick Launcher. Searching only reads caches in the native side (never the disk), so it is safe to
   * call on every debounced keystroke. Nothing typed here leaves the PC except through an explicit
   * web search or AI command.
   */
  launcherSearch(query: string): Promise<LauncherSearchResult>;
  /** Run a row. `input` is the text that produced it; the native side re-checks the pair. Rejects with a readable reason. */
  launcherRun(input: string, target: LauncherTarget, reveal?: boolean): Promise<LauncherRunOutcome>;
  launcherHide(): Promise<void>;
  launcherState(): Promise<LauncherView>;
  launcherSetPin(id: string, pinned: boolean): Promise<void>;
  launcherRecordSearch(text: string): Promise<void>;
  /** Forget recent launches and search history (pins stay). */
  launcherClearHistory(): Promise<void>;
  /** Re-scan apps and files in the background. */
  launcherRefresh(): Promise<void>;
  /** Small PNG data URL, or null (the page shows a lettered tile). */
  launcherAppIcon(id: string): Promise<string | null>;
  launcherAiInfo(): Promise<LauncherAiInfo>;
  /** Ask the configured provider once. The answer streams through `onLauncherAi`. */
  launcherAiSend(kind: LauncherAiKind, text: string, language: string | null): Promise<number>;
  onLauncherAi(cb: (e: AiEvent) => void): Promise<Unsub>;
  /** Native window events: shown, asked to close (shortcut pressed again), index refreshed. */
  onLauncherOpen(cb: () => void): Promise<Unsub>;
  onLauncherClose(cb: () => void): Promise<Unsub>;
  onLauncherIndex(cb: () => void): Promise<Unsub>;

  /**
   * Widgets Hub. Everything stays on this PC. The layout and each widget's data are saved natively
   * (validated there); timers live in the native side so they keep running when the Hub is closed.
   */
  /** Open the Hub window (or focus it), optionally scrolled to one widget. */
  hubOpen(widget?: string): Promise<void>;
  /** The widgets this build offers. */
  hubCatalog(): Promise<WidgetDef[]>;
  hubLayout(): Promise<HubLayout>;
  /** Validates and saves; resolves with the layout that was actually kept. */
  hubSaveLayout(layout: HubLayout): Promise<HubLayout>;
  onHubChanged(cb: (l: HubLayout) => void): Promise<Unsub>;
  /** Which widget to scroll to when an already-open Hub is asked to show one. */
  onHubFocus(cb: (kind: string) => void): Promise<Unsub>;
  /** One JSON document per widget kind. `null` when nothing was saved yet. Rejects with a readable reason. */
  hubDataGet(kind: string): Promise<unknown>;
  hubDataSet(kind: string, value: unknown): Promise<void>;
  /** A widget's line on the island (null clears it). */
  hubSetStatus(kind: string, status: HubStatus | null): Promise<void>;
  hubStatuses(): Promise<HubStatusEntry[]>;
  onHubStatus(cb: (s: HubStatusEntry[]) => void): Promise<Unsub>;

  /** Shared timers (absolute end times). All operations reject with a readable reason. */
  timersList(): Promise<HubTimer[]>;
  timerCreate(owner: string, label: string, durationMs: number): Promise<number>;
  timerStart(id: number): Promise<void>;
  timerPause(id: number): Promise<void>;
  timerReset(id: number): Promise<void>;
  timerDelete(id: number): Promise<void>;
  timerSetDuration(id: number, durationMs: number): Promise<void>;
  timerSetLabel(id: number, label: string): Promise<void>;
  onTimers(cb: (t: HubTimer[]) => void): Promise<Unsub>;
  onTimerFinished(cb: (f: TimerFinished) => void): Promise<Unsub>;
}
