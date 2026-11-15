import { useCallback, useEffect, useMemo, useState } from 'react';
import type { LyricsDoc, MediaSession } from '../shared/types';
import { clearLocalLrc, localLrcProvider, saveLocalLrc, trackId, trackKeyOf, type LyricsProvider } from './provider';

export type LyricsStatus = 'no-track' | 'loading' | 'ready' | 'not-found';

export interface LyricsHook {
  doc: LyricsDoc | null;
  status: LyricsStatus;
  error: string | null;
  /** Use a .lrc file's text for the current track. Returns an error message or null. */
  loadFromText(text: string): string | null;
  clear(): void;
}

const DEFAULT_PROVIDERS: LyricsProvider[] = [localLrcProvider];

export function useLyrics(session: MediaSession | null, providers: LyricsProvider[] = DEFAULT_PROVIDERS): LyricsHook {
  const track = useMemo(() => (session ? trackKeyOf(session) : null), [session?.title, session?.artist, session?.album, session?.durationMs]); // eslint-disable-line react-hooks/exhaustive-deps
  const id = track ? trackId(track) : null;
  const [doc, setDoc] = useState<LyricsDoc | null>(null);
  const [status, setStatus] = useState<LyricsStatus>('no-track');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setError(null);
    if (!track) {
      setDoc(null);
      setStatus('no-track');
      return;
    }
    let cancelled = false;
    setDoc(null);
    setStatus('loading');
    (async () => {
      // Fallback chain: first provider that has synced lyrics wins; failures are skipped.
      for (const p of providers) {
        try {
          const found = await p.fetch(track);
          if (cancelled) return;
          if (found) {
            setDoc(found);
            setStatus('ready');
            return;
          }
        } catch {
          /* try next provider */
        }
      }
      if (!cancelled) setStatus('not-found');
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, providers]);

  const loadFromText = useCallback(
    (text: string) => {
      if (!track) return 'Nothing is playing.';
      const r = saveLocalLrc(track, text);
      if (!r.ok) {
        setError(r.error);
        return r.error;
      }
      setError(null);
      setDoc(r.doc);
      setStatus('ready');
      return null;
    },
    [track],
  );

  const clear = useCallback(() => {
    if (track) clearLocalLrc(track);
    setDoc(null);
    setStatus(track ? 'not-found' : 'no-track');
  }, [track]);

  return { doc, status, error, loadFromText, clear };
}
