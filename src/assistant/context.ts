import type { LyricsDoc, MediaSession } from '../shared/types';

/** Which pieces of app context a message asks about. */
export interface ContextWants {
  song: boolean;
  lyrics: boolean;
}

/**
 * Does the message ask about the music playing right now? Decided from the user's own words, so nothing
 * about the music is attached to unrelated questions. Phrasings are matched by meaning, not by one
 * fixed sentence: "what song is this", "what am I listening to", "who sings this", "what's playing"…
 */
const SONG_NOUN = '(?:song|track|tune|music|single)';
const THIS_MUSIC = new RegExp(`\\b(?:this|the\\s+current|current|the\\s+playing|the\\s+currently\\s+playing)\\s+(?:${SONG_NOUN}|album|artist|singer|band)\\b`, 'i');
const SONG_PATTERNS: RegExp[] = [
  // "what song is this", "which track is playing", "what song am I currently listening to"
  new RegExp(`\\b(?:what|which)\\s+${SONG_NOUN}\\b[^.?!]{0,30}\\b(?:is|am\\s+i|are\\s+we|'?s)\\b[^.?!]{0,25}\\b(?:playing|listening|hearing|this|that|right\\s+now|now)\\b`, 'i'),
  // "what am I listening to", "what are we hearing"
  /\bwhat\s+(?:am\s+i|are\s+we)\s+(?:currently\s+|now\s+)?(?:listening\s+to|hearing)\b/i,
  // "what's playing", "what is playing now" (but not "what's playing at the cinema")
  /\bwhat(?:'?s|\s+is)\s+(?:currently\s+|now\s+)?playing\b(?!\s+(?:at|in|on|near|tonight|tomorrow|today|this\s+(?:week|weekend)))/i,
  /\b(?:now|currently)\s+playing\b/i,
  // who is it
  /\bwho\s+(?:sings|sang|is\s+singing)\s*(?:this|it|that|the\s+song)?\s*[?.!]*\s*$/i,
  /\bwho\s+(?:wrote|performs|made|produced|sings|sang)\s+(?:this|the\s+(?:song|track))\b/i,
  /\bwho(?:'?s|\s+is)\s+(?:this|it)\s+by\b/i,
  /\bwho(?:'?s|\s+is)\s+the\s+(?:artist|singer|band|musician)\b/i,
  // state of the music
  /\b(?:is|was)\s+(?:the\s+)?(?:music|song|track)\s+(?:paused|playing|stopped)\b/i,
];

const LYRIC_PATTERNS: RegExp[] = [
  /\b(?:these|this|the|current|its|their|those)\s+lyrics\b/i,
  /\bwhat\s+(?:is|are)\s+(?:he|she|they|the\s+(?:singer|artist))\s+(?:saying|singing)\b/i,
  /\bwhat\s+(?:does|do)\s+(?:he|she|they)\s+(?:say|sing)\b/i,
  // meaning / interpretation of the current song
  /\bwhat\s+(?:does|do)\s+(?:this|the\s+current)\s+(?:song|track|lyrics?)\s+mean\b/i,
  /\bwhat(?:'?s|\s+is)\s+(?:this|the\s+current)\s+(?:song|track)\s+about\b/i,
  /\b(?:meaning|message|theme|story)\s+of\s+(?:this|the\s+current)\s+(?:song|track|lyrics?)\b/i,
  /\b(?:explain|interpret|analy[sz]e|translate|summari[sz]e)\s+(?:this|the\s+current)\s+(?:song|track)\b/i,
  /\bthis\s+(?:verse|chorus)\b/i,
];

/** Asking VEX to invent a song or lyrics (not to read the current one). */
const CREATIVE = /\b(?:write|compose|create|generate|make|draft|invent|come\s+up\s+with|produce)\b/i;

/** Decide from the user's own words whether they are asking about the music. Nothing is attached otherwise. */
export function detectContextRequest(text: string): ContextWants {
  const deictic = THIS_MUSIC.test(text);
  // "write a song about cats" / "write lyrics for…" is a creative request, unless it points at the current song.
  if (CREATIVE.test(text) && !deictic && !/\b(?:these|current)\s+lyrics\b/i.test(text)) return { song: false, lyrics: false };
  const lyrics = LYRIC_PATTERNS.some((r) => r.test(text));
  const song = lyrics || deictic || SONG_PATTERNS.some((r) => r.test(text));
  return { song, lyrics };
}

const mmss = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
};

export const LYRICS_CONTEXT_CHARS = 6000;

export interface ContextSources {
  session: MediaSession | null;
  lyrics: LyricsDoc | null;
  /** Unsynced lyrics text, when that is all that was found. */
  plainLyrics?: string | null;
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
    } else if (!src.lyrics && src.plainLyrics) {
      let body = src.plainLyrics.trim();
      if (body.length > LYRICS_CONTEXT_CHARS) body = body.slice(0, LYRICS_CONTEXT_CHARS) + '\n…(truncated)';
      parts.push(`Lyrics (not time-synced):\n${body}`);
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
