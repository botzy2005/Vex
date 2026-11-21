import type { ReactNode } from 'react';
import type { LauncherRow } from '../shared/types';

const base = { width: 18, height: 18, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 1.7, strokeLinecap: 'round', strokeLinejoin: 'round', 'aria-hidden': true } as const;
const svg = (children: ReactNode) => <svg {...base}>{children}</svg>;

export const SearchIcon = () => (
  <svg {...base} width={20} height={20}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="m16 16 4 4" />
  </svg>
);
export const StarIcon = ({ filled }: { filled?: boolean }) => (
  <svg {...base} width={15} height={15} fill={filled ? 'currentColor' : 'none'}>
    <path d="m12 3.5 2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.8 6.8 19.6l1-5.8L3.5 9.7l5.9-.9z" />
  </svg>
);
export const CloseIcon = () => (
  <svg {...base} width={14} height={14}>
    <path d="m6 6 12 12M18 6 6 18" />
  </svg>
);
export const Spinner = () => <span className="lx-spinner" aria-hidden="true" />;

const folder = svg(<path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2.2h7a2 2 0 0 1 2 2v7.3a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />);
const doc = svg(<><path d="M7 3.5h7l4 4v13H7z" /><path d="M14 3.5v4h4M9.5 12.5h5M9.5 15.5h5" /></>);
const image = svg(<><rect x="4" y="5" width="16" height="14" rx="2" /><circle cx="9" cy="10" r="1.5" /><path d="m5 17 4.5-4.5 3 3 2-2L19 17" /></>);
const video = svg(<><rect x="3.5" y="6" width="12" height="12" rx="2" /><path d="m15.5 10.5 5-2.5v8l-5-2.5" /></>);
const audio = svg(<><path d="M9 17.5V6l9-2v11.5" /><circle cx="6.8" cy="17.5" r="2.2" /><circle cx="15.8" cy="15.5" r="2.2" /></>);
const archive = svg(<><rect x="4" y="4.5" width="16" height="15" rx="2" /><path d="M12 4.5v9M10.5 7h3M10.5 10h3" /></>);
const code = svg(<path d="m9 8-4 4 4 4M15 8l4 4-4 4M13.5 6l-3 12" />);
const other = svg(<><path d="M7 3.5h7l4 4v13H7z" /><path d="M14 3.5v4h4" /></>);
const calc = svg(<><rect x="5" y="3.5" width="14" height="17" rx="2" /><path d="M8.5 8h7M9 12.5h.01M12 12.5h.01M15 12.5h.01M9 16h.01M12 16h.01M15 16h.01" /></>);
const clock = svg(<><circle cx="12" cy="12" r="8" /><path d="M12 7.5V12l3 2" /></>);
const calendar = svg(<><rect x="4" y="5.5" width="16" height="14.5" rx="2" /><path d="M4 10h16M8.5 3.5v4M15.5 3.5v4" /></>);
const gear = svg(<><circle cx="12" cy="12" r="3" /><path d="M12 3.5v2.2M12 18.3v2.2M3.5 12h2.2M18.3 12h2.2M6 6l1.6 1.6M16.4 16.4 18 18M6 18l1.6-1.6M16.4 7.6 18 6" /></>);
const globe = svg(<><circle cx="12" cy="12" r="8.5" /><path d="M3.5 12h17M12 3.5c2.5 2.4 3.5 5.2 3.5 8.5s-1 6.100-3.500 8.500c-2.500-2.400-3.500-5.200-3.500-8.500s1-6.100 3.500-8.500z" /></>);
const sparkle = svg(<path d="M12 3.5 13.8 9l5.7 1.8-5.7 1.8L12 18.500l-1.800-5.900L4.500 10.800 10.200 9zM18.500 16.500l.7 1.800 1.800.7-1.800.7-.7 1.800-.7-1.800-1.800-.7 1.800-.7z" />);
const note = svg(<><path d="M6 4.500h12v15H6z" /><path d="M9 9h6M9 12.500h6M9 16h3.500" /></>);
const timer = svg(<><circle cx="12" cy="13" r="7" /><path d="M12 9.500V13l2.200 1.500M9.500 3.500h5" /></>);

const FILE_GLYPH: Record<string, ReactNode> = { folder, document: doc, image, video, audio, archive, code, other };

export function RowGlyph({ row }: { row: LauncherRow }) {
  if (row.category === 'file' || row.category === 'folder') return <>{FILE_GLYPH[row.fileKind ?? 'other'] ?? other}</>;
  if (row.category === 'web') return globe;
  if (row.category === 'ai') return sparkle;
  const t = row.target.type === 'command' ? row.target.action.type : '';
  switch (t) {
    case 'calc':
      return calc;
    case 'time':
      return clock;
    case 'date':
      return calendar;
    case 'openFolder':
      return folder;
    case 'openVexSettings':
    case 'openWindowsSettings':
      return gear;
    case 'timer':
      return timer;
    case 'note':
      return note;
    default:
      return sparkle;
  }
}

/** A short human label for the file-type badge. */
export function kindLabel(row: LauncherRow): string | null {
  if (row.category === 'folder') return 'Folder';
  if (row.category === 'file') {
    const ext = row.title.includes('.') ? row.title.split('.').pop()!.toUpperCase() : '';
    return ext && ext.length <= 5 ? ext : (row.fileKind ?? 'file').toString().replace(/^./, (c) => c.toUpperCase());
  }
  return null;
}
