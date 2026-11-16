import { useCallback, useEffect, useMemo, useState } from 'react';
import type { Backend } from '../backend/backend';
import type { LyricsDoc, MediaSession } from '../shared/types';
import { onlineProvider } from './online';
import {
  clearLocalLrc,
  localLrcProvider,
  saveLocalLrc,
  trackId,
  trackKeyOf,
  type LyricsOutcome,
  type LyricsProvider,
} from './provider';

export type LyricsStatus = 'no-track' | 'loading' | 'ready' | 'plain' | 'instrumental' | 'not-found' | 'error';

export interface LyricsHook {
  doc: LyricsDoc | null;
  /** Unsynced lyrics text, shown statically when nothing synced was found. */
  plain: string | null;
  source: string | null;
  status: LyricsStatus;
  /** False when automatic online lookup is switched off in settings. */
  online: boolean;
  error: string | null;
  /** Use a .lrc file's text for the current track. Returns an error message or null. */
  loadFromText(text: string): string | null;
  clear(): void;
  /** Ask the online provider again (after a network failure). */
  retry(): void;
}

const AUTO_RETRY_MS = 20_000;

export function useLyrics(session: MediaSession | null, backend: Backend, online: boolean): LyricsHook {
  const track = useMemo(() => (session ? trackKeyOf(session) : null), [session?.title, session?.artist, session?.album, session?.durationMs]); // eslint-disable-line react-hooks/exhaustive-deps
  const id = track ? trackId(track) : null;
  const providers = useMemo<LyricsProvider[]>(
    () => (online ? [localLrcProvider, onlineProvider(backend)] : [localLrcProvider]),
    [backend, online],
  );
  const [outcome, setOutcome] = useState<LyricsOutcome | null>(null);
  const [status, setStatus] = useState<LyricsStatus>('no-track');
  const [error, setError] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);

  useEffect(() => {
    setError(null);
    setOutcome(null);
    if (!track) {
      setStatus('no-track');
      return;
    }
    let cancelled = false;
    let retryTimer: number | undefined;
    setStatus('loading');
    (async () => {
      let failed = false;
      let fallback: LyricsOutcome | null = null; // plain/instrumental result, used if nothing synced turns up
      for (const p of providers) {
        try {
          const found = await p.fetch(track);
          if (cancelled) return;
          if (found?.kind === 'synced') {
            setOutcome(found);
            setStatus('ready');
            return;
          }
          if (found && !fallback) fallback = found;
        } catch {
          failed = true; // try the next provider
        }
      }
      if (cancelled) return;
      if (fallback) {
        setOutcome(fallback);
        setStatus(fallback.kind === 'plain' ? 'plain' : 'instrumental');
      } else if (failed) {
        setStatus('error');
        retryTimer = window.setTimeout(() => !cancelled && setNonce((n) => n + 1), AUTO_RETRY_MS);
      } else {
        setStatus('not-found');
      }
    })();
    return () => {
      cancelled = true;
      window.clearTimeout(retryTimer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, providers, nonce]);

  const loadFromText = useCallback(
    (text: string) => {
      if (!track) return 'Nothing is playing.';
      const r = saveLocalLrc(track, text);
      if (!r.ok) {
        setError(r.error);
        return r.error;
      }
      setError(null);
      setOutcome({ kind: 'synced', doc: r.doc });
      setStatus('ready');
      return null;
    },
    [track],
  );

  const clear = useCallback(() => {
    if (track) clearLocalLrc(track);
    setNonce((n) => n + 1); // re-run the chain: an online result may now take over
  }, [track]);

  const retry = useCallback(() => setNonce((n) => n + 1), []);

  return {
    doc: outcome?.kind === 'synced' ? outcome.doc : null,
    plain: outcome?.kind === 'plain' ? outcome.text : null,
    source: outcome ? (outcome.kind === 'synced' ? outcome.doc.source : outcome.source) : null,
    status,
    online,
    error,
    loadFromText,
    clear,
    retry,
  };
}
