import type { Backend } from '../backend/backend';
import { parseLrc } from './lrc';
import type { LyricsProvider } from './provider';

/** Automatic lookup on LRCLIB, performed natively (see src-tauri/src/lyrics.rs). */
export function onlineProvider(backend: Backend): LyricsProvider {
  return {
    id: 'lrclib',
    name: 'LRCLIB',
    async fetch(track) {
      const r = await backend.lyricsFetch({
        title: track.title,
        artist: track.artist,
        album: track.album,
        durationMs: track.durationMs,
      });
      if (!r) return null;
      if (r.synced) {
        const doc = parseLrc(r.synced, r.source);
        if (doc) return { kind: 'synced', doc };
      }
      if (r.instrumental) return { kind: 'instrumental', source: r.source };
      if (r.plain && r.plain.trim()) return { kind: 'plain', text: r.plain, source: r.source };
      return null;
    },
  };
}
