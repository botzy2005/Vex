import type { LyricsDoc, MediaSession } from '../shared/types';

/** Which pieces of app context a message asks about. */
export interface ContextWants {
  song: boolean;
  lyrics: boolean;
}

const SONG = /\b(what('?s| is)\s+(this\s+|the\s+)?(song|track|music)|which\s+(song|track)|(song|track|music)\s+is\s+(playing|on)|now\s+playing|currently\s+playing|who\s+(sings|sang|is\s+singing|is\s+this\s+by|made\s+this)|(this|current|playing)\s+(song|track)|the\s+(song|track)\s+(i'?m|that'?s)|artist\s+of)\b/i;
const LYRICS = /\b(lyrics?|the\s+words|what\s+(is|are)\s+(he|she|they)\s+(saying|singing)|(this|current|these|those)\s+(line|lines|verse|chorus|lyrics?)|meaning\s+of\s+(this|the)\s+(song|track))\b/i;

/** Decide from the user's own words whether they are asking about the music. Nothing is attached otherwise. */
export function detectContextRequest(text: string): ContextWants {
  const lyrics = LYRICS.test(text);
  return { lyrics, song: lyrics || SONG.test(text) };
}

const mmss = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
};

export const LYRICS_CONTEXT_CHARS = 6000;

export interface ContextSources {
  session: MediaSession | null;
  lyrics: LyricsDoc | null;
  /** Index of the line being sung, or -1. */
  activeIndex: number;
  /** Position now, in ms. */
  positionMs: number;
}

/**
 * Build the text that is attached to ONE message. Returns null when nothing was asked for.
 * When something was asked for but isn't available (no song, no lyrics), it says so honestly
 * instead of leaving the model to guess.
 */
export function buildContext(wants: ContextWants, src: ContextSources): { label: string; text: string } | null {
  if (!wants.song && !wants.lyrics) return null;
  const parts: string[] = [];
  const labels: string[] = [];
  const s = src.session;

  if (wants.song || wants.lyrics) {
    labels.push('Now playing');
    if (!s) {
      parts.push('Now playing: nothing is playing right now.');
    } else {
      const bits = [`Now playing: "${s.title}" by ${s.artist || 'an unknown artist'}`];
      if (s.album) bits.push(`from the album "${s.album}"`);
      const state = s.status === 'playing' ? 'playing' : s.status === 'paused' ? 'paused' : 'stopped';
      const at = s.durationMs != null ? `${mmss(src.positionMs)} of ${mmss(s.durationMs)}` : mmss(src.positionMs);
      parts.push(`${bits.join(' ')}. Playback is ${state} (${at}).`);
    }
  }

  if (wants.lyrics) {
    labels.push('Lyrics');
    if (!s) {
      parts.push('Lyrics: unavailable because nothing is playing.');
    } else if (!src.lyrics) {
      parts.push('Lyrics: VEX has no lyrics loaded for this track.');
    } else {
      const lines = src.lyrics.lines.map((l) => l.text).filter((t) => t.trim());
      let body = lines.join('\n');
      if (body.length > LYRICS_CONTEXT_CHARS) body = body.slice(0, LYRICS_CONTEXT_CHARS) + '\n…(truncated)';
      const current = src.activeIndex >= 0 ? src.lyrics.lines[src.activeIndex]?.text : '';
      parts.push(`Lyrics (from ${src.lyrics.source})${current ? `, line being sung now: "${current}"` : ''}:\n${body}`);
    }
  }
  return { label: labels.join(' + '), text: `[App context shared by VEX because the user asked about it]\n${parts.join('\n')}` };
}
