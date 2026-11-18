import type { ClipboardItem, ClipboardStatus } from '../shared/types';

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

/** The small grey line under an entry: size for images, length for text. */
export function describeItem(i: ClipboardItem): string {
  if (i.kind === 'image') return `Image · ${i.width}×${i.height} · ${formatBytes(i.bytes)}`;
  const parts = [`${i.chars.toLocaleString('en-US')} character${i.chars === 1 ? '' : 's'}`];
  if (i.lines > 1) parts.unshift(`${i.lines} lines`);
  return parts.join(' · ');
}

export type ClipTone = 'off' | 'paused' | 'recording';

/** What the user is told about whether the history is listening, so it is never silently active. */
export function statusLine(s: ClipboardStatus): { tone: ClipTone; text: string } {
  if (!s.enabled) return { tone: 'off', text: 'Clipboard history is off' };
  if (s.paused) return { tone: 'paused', text: `Paused · not recording · ${s.count} saved` };
  return { tone: 'recording', text: `Recording copies · ${s.count} saved` };
}

export function sensitiveNote(skipped: number): string | null {
  if (skipped <= 0) return null;
  return `${skipped} ${skipped === 1 ? 'copy' : 'copies'} that looked private ${skipped === 1 ? 'was' : 'were'} not saved.`;
}
