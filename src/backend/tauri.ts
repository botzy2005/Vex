import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Backend } from './backend';

export function createTauriBackend(): Backend {
  const sub = <T,>(event: string, cb: (payload: T) => void) => listen<T>(event, (e) => cb(e.payload));
  return {
    kind: 'tauri',
    getSettings: () => invoke('get_settings'),
    updateSettings: (patch) => invoke('update_settings', { patch }),
    onSettingsChanged: (cb) => sub('settings-changed', cb),
    listMonitors: () => invoke('list_monitors'),
    setIslandSize: (width, height) => invoke('set_island_size', { width, height }),
    openSettings: () => invoke('open_settings'),
    quit: () => invoke('quit_app'),
    onIslandToggle: (cb) => sub<null>('island-toggle', () => cb()),
    getMedia: () => invoke('media_get_state'),
    onMedia: (cb) => sub('media-state', cb),
    mediaTransport: (command) => invoke('media_transport', { command }),
    lyricsFetch: (track) => invoke('lyrics_fetch', { track }),
    aiSend: ({ conversationId, text, context }) => invoke('ai_send', { conversationId, text, context }),
    aiCancel: () => invoke('ai_cancel'),
    onAiEvent: (cb) => sub('ai-event', cb),
    aiKeyStatus: (provider) => invoke('ai_key_status', { provider }),
    aiSetKey: (provider, key) => invoke('ai_set_key', { provider, key }),
    aiClearKey: (provider) => invoke('ai_clear_key', { provider }),
    aiListModels: (provider) => invoke('ai_list_models', { provider }),
    aiTest: (provider) => invoke('ai_test', { provider }),
    aiMetrics: () => invoke('ai_metrics'),
    aiMetricsClear: () => invoke('ai_metrics_clear'),
    aiPreload: (model) => invoke('ai_preload', { model }),
    aiBenchmark: (models, includeThinking) => invoke('ai_benchmark', { models, includeThinking }),
    onBenchRow: (cb) => sub('ai-bench-row', cb),
    chatList: () => invoke('chat_list'),
    chatGet: (id) => invoke('chat_get', { id }),
    chatDelete: (id) => invoke('chat_delete', { id }),
    chatClearAll: () => invoke('chat_clear_all'),
    assistantPreview: (state) => invoke('assistant_preview_state', { state }),
    onAssistantState: (cb) => sub('assistant-state', cb),
  };
}
