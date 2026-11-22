import type { SVGProps } from 'react';

const base = (p: SVGProps<SVGSVGElement>) => ({
  width: 18,
  height: 18,
  viewBox: '0 0 24 24',
  fill: 'currentColor',
  'aria-hidden': true as const,
  ...p,
});

export const PlayIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M8 5.5v13a1 1 0 0 0 1.52.85l10.4-6.5a1 1 0 0 0 0-1.7L9.52 4.65A1 1 0 0 0 8 5.5Z" /></svg>
);
export const PauseIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><rect x="6.5" y="5" width="4" height="14" rx="1.2" /><rect x="13.5" y="5" width="4" height="14" rx="1.2" /></svg>
);
export const NextIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M6 6.4v11.2a.9.9 0 0 0 1.4.75l8.2-5.6a.9.9 0 0 0 0-1.5L7.4 5.65A.9.9 0 0 0 6 6.4Z" /><rect x="16.5" y="5.5" width="2.6" height="13" rx="1.1" /></svg>
);
export const PrevIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M18 6.4v11.2a.9.9 0 0 1-1.4.75l-8.2-5.6a.9.9 0 0 1 0-1.5l8.2-5.6A.9.9 0 0 1 18 6.4Z" /><rect x="4.9" y="5.5" width="2.6" height="13" rx="1.1" /></svg>
);
export const MusicIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M16 3.5v10.1A3.4 3.4 0 1 0 17.8 16.6V8h2.7V3.5H16Z" /></svg>
);
export const LyricsIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M4 7h16M4 12h11M4 17h14" /></svg>
);
export const SparkIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M12 2.5c.5 4.6 2.4 7 7 7.5-4.6.5-6.5 2.9-7 7.5-.5-4.6-2.4-7-7-7.5 4.600-.500 6.500-2.900 7-7.500Zm7 12.5c.2 1.900 1 2.800 2.800 3-1.800.2-2.600 1.100-2.800 3-.2-1.900-1-2.800-2.800-3 1.800-.2 2.600-1.100 2.800-3Z" /></svg>
);
export const GearIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3.2" />
    <path d="M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.5 5.5l1.7 1.7M16.8 16.8l1.7 1.7M5.5 18.5l1.7-1.7M16.8 7.2l1.7-1.7" />
  </svg>
);
export const PinIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M15.2 3.5 20.5 8.8l-1.4 1.4-1.2-.3-3.300 3.300.5 4-1.400 1.400-3.300-3.300-5 5-1.100-1.100 5-5-3.300-3.300 1.400-1.400 4 .5 3.300-3.300-.3-1.200 1.400-1.400Z" /></svg>
);
export const PowerIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M12 3v8M6.6 6.8a8 8 0 1 0 10.800 0" /></svg>
);
export const MicIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5.5 11.500a6.500 6.500 0 0 0 13 0M12 18v3" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /></svg>
);
export const SendIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M3.4 20.4 21 12 3.400 3.600l-.1 6.500L15 12 3.300 13.900l.1 6.500Z" /></svg>
);
export const PlusIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" /></svg>
);
export const ClockIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><circle cx="12" cy="12" r="8.500" fill="none" stroke="currentColor" strokeWidth="1.8" /><path d="M12 7.500V12l3 2" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>
);
export const StopIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><rect x="6.500" y="6.500" width="11" height="11" rx="2.500" /></svg>
);
export const TrashIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}><path d="M5 7h14M10 7V5h4v2M7 7l.8 12h8.400L17 7M10.500 11v5M13.500 11v5" fill="none" stroke="currentColor" strokeWidth="1.700" strokeLinecap="round" strokeLinejoin="round" /></svg>
);
export const BellIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 1.5h-15L6 16.5ZM10 20.5a2 2 0 0 0 4 0" /></svg>
);
export const ClipboardIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="6" y="5" width="12" height="16" rx="2" /><path d="M9 5V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v1M9.5 11h5M9.5 15h5" /></svg>
);
export const CloseIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M6 6l12 12M18 6 6 18" /></svg>
);
export const CameraIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M4 8.500A1.500 1.500 0 0 1 5.500 7H8l1.200-2h5.600L16 7h2.500A1.500 1.500 0 0 1 20 8.500v9a1.500 1.500 0 0 1-1.500 1.500h-13A1.500 1.500 0 0 1 4 17.500v-9Z" /><circle cx="12" cy="13" r="3.200" /></svg>
);

/** Speaker with 0-3 sound waves, or a cross when muted. */
export const VolumeIcon = ({ level, ...p }: SVGProps<SVGSVGElement> & { level: 'muted' | 'low' | 'medium' | 'high' }) => (
  <svg {...base(p)}>
    <path d="M4 9.5v5h3.2l4.3 3.6a.8.8 0 0 0 1.3-.6V6.5a.8.8 0 0 0-1.3-.6L7.2 9.5H4Z" />
    {level === 'muted' && <path d="m16 9.5 4.5 5m0-5-4.5 5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />}
    {level !== 'muted' && <path d="M15.5 9.4a3.6 3.6 0 0 1 0 5.2" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />}
    {(level === 'medium' || level === 'high') && <path d="M17.9 7.3a6.8 6.8 0 0 1 0 9.4" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />}
    {level === 'high' && <path d="M20.2 5.2a9.8 9.8 0 0 1 0 13.6" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />}
  </svg>
);
