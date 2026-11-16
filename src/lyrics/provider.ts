import type { LyricsDoc, MediaSession } from '../shared/types';
import { parseLrc } from './lrc';

export interface TrackKey {
  title: string;
  artist: string;
  album?: string;
  durationMs: number | null;
}

/** What a provider found for a track. */
export type LyricsOutcome =
  | { kind: 'synced'; doc: LyricsDoc }
  /** Lyrics exist but carry no timing, so they cannot be followed live. */
  | { kind: 'plain'; text: string; source: string }
  | { kind: 'instrumental'; source: string };

/** A source of lyrics. Providers are tried in order; the first synced result wins. */
export interface LyricsProvider {
  id: string;
  name: string;
  fetch(track: TrackKey): Promise<LyricsOutcome | null>;
}

export const trackKeyOf = (s: MediaSession): TrackKey => ({
  title: s.title,
  artist: s.artist,
  album: s.album,
  durationMs: s.durationMs,
});

export const trackId = (t: TrackKey) => `${t.artist}\u0000${t.title}\u0000${t.album ?? ''}`.toLowerCase();

const STORE_PREFIX = 'islet.lrc.';
const MAX_BYTES = 512 * 1024;

/** Lyrics files the user picked themselves, remembered per track on this device. */
export const localLrcProvider: LyricsProvider = {
  id: 'local-lrc',
  name: 'Your .lrc files',
  async fetch(track) {
    try {
      const text = localStorage.getItem(STORE_PREFIX + trackId(track));
      const doc = text ? parseLrc(text, 'Your .lrc file') : null;
      return doc ? { kind: 'synced', doc } : null;
    } catch {
      return null;
    }
  },
};

export type SaveResult = { ok: true; doc: LyricsDoc } | { ok: false; error: string };

export function saveLocalLrc(track: TrackKey, text: string): SaveResult {
  if (text.length > MAX_BYTES) return { ok: false, error: 'That file is too large to be a lyrics file.' };
  const doc = parseLrc(text, 'Your .lrc file');
  if (!doc) return { ok: false, error: 'No timestamps found. Synced lyrics need lines like [00:12.50] text.' };
  try {
    localStorage.setItem(STORE_PREFIX + trackId(track), text);
  } catch {
    /* still usable for this session */
  }
  return { ok: true, doc };
}

export function clearLocalLrc(track: TrackKey): void {
  try {
    localStorage.removeItem(STORE_PREFIX + trackId(track));
  } catch {
    /* ignore */
  }
}
