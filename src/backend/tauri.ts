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
    assistantSend: (text) => invoke('assistant_send', { text }),
    assistantPreview: (state) => invoke('assistant_preview_state', { state }),
    onAssistantState: (cb) => sub('assistant-state', cb),
  };
}
