import type { SVGProps } from 'react';

const base = (p: SVGProps<SVGSVGElement>) => ({
  width: 16,
  height: 16,
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.9,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
  'aria-hidden': true as const,
  ...p,
});
type P = SVGProps<SVGSVGElement>;

export const GridIcon = (p: P) => (
  <svg {...base(p)}><rect x="4" y="4" width="7" height="7" rx="1.8" /><rect x="13" y="4" width="7" height="7" rx="1.8" /><rect x="4" y="13" width="7" height="7" rx="1.8" /><rect x="13" y="13" width="7" height="7" rx="1.8" /></svg>
);
export const CompassIcon = (p: P) => (
  <svg {...base(p)}><circle cx="12" cy="12" r="8.5" /><path d="m15.2 8.8-1.7 4.7-4.700 1.700 1.700-4.700 4.700-1.700Z" /></svg>
);
export const PlusIcon = (p: P) => (
  <svg {...base(p)}><path d="M12 5v14M5 12h14" /></svg>
);
export const CloseIcon = (p: P) => (
  <svg {...base(p)}><path d="m6 6 12 12M18 6 6 18" /></svg>
);
export const CheckIcon = (p: P) => (
  <svg {...base(p)}><path d="m5 12.500 4.500 4.500L19 7.500" /></svg>
);
export const GripIcon = (p: P) => (
  <svg {...base(p)} fill="currentColor" stroke="none"><circle cx="9" cy="6.500" r="1.500" /><circle cx="15" cy="6.500" r="1.500" /><circle cx="9" cy="12" r="1.500" /><circle cx="15" cy="12" r="1.500" /><circle cx="9" cy="17.500" r="1.500" /><circle cx="15" cy="17.500" r="1.500" /></svg>
);
export const ChevronLeftIcon = (p: P) => (
  <svg {...base(p)}><path d="m14.500 6-6 6 6 6" /></svg>
);
export const ChevronRightIcon = (p: P) => (
  <svg {...base(p)}><path d="m9.500 6 6 6-6 6" /></svg>
);
export const SlidersIcon = (p: P) => (
  <svg {...base(p)}><path d="M4 7h9M17 7h3M4 17h3M11 17h9" /><circle cx="15" cy="7" r="2" /><circle cx="9" cy="17" r="2" /></svg>
);
export const ResetIcon = (p: P) => (
  <svg {...base(p)}><path d="M5 12a7 7 0 1 0 2.200-5.100" /><path d="M4.500 4.500v4h4" /></svg>
);
export const PinFilledIcon = (p: P) => (
  <svg {...base(p)} fill="currentColor" stroke="none"><path d="M15.200 3.500 20.500 8.800l-1.400 1.400-1.200-.3-3.300 3.300.5 4-1.400 1.400-3.300-3.300-5 5-1.100-1.100 5-5-3.300-3.300 1.400-1.400 4 .5 3.300-3.300-.3-1.200 1.400-1.400Z" /></svg>
);
export const PinOutlineIcon = (p: P) => (
  <svg {...base(p)} strokeWidth="1.6"><path d="M15.200 3.500 20.500 8.800l-1.400 1.400-1.200-.3-3.300 3.300.5 4-1.400 1.400-3.300-3.300-5 5-1.100-1.100 5-5-3.300-3.300 1.400-1.400 4 .5 3.300-3.300-.3-1.200 1.400-1.400Z" /></svg>
);
export const TimerIcon = (p: P) => (
  <svg {...base(p)}><circle cx="12" cy="13.500" r="7" /><path d="M12 9.500v4l2.500 1.500M9.500 3.500h5" /></svg>
);
export const NoteIcon = (p: P) => (
  <svg {...base(p)}><path d="M6 3.5h9l3.5 3.5v13a.5.5 0 0 1-.5.5H6a.5.5 0 0 1-.5-.5V4a.5.5 0 0 1 .5-.5Z" /><path d="M14.5 3.5V7.5h4M8.5 12h7M8.5 15.5h5" /></svg>
);
export const GlobeIcon = (p: P) => (
  <svg {...base(p)}><circle cx="12" cy="12" r="8.5" /><path d="M3.5 12h17M12 3.5c2.6 2.4 3.9 5.2 3.9 8.500S14.600 18 12 20.500C9.400 18 8.100 15.200 8.100 12S9.400 5.900 12 3.500Z" /></svg>
);
export const SunIcon = (p: P) => (
  <svg {...base(p)}><circle cx="12" cy="12" r="3.8" /><path d="M12 3.500v2M12 18.500v2M3.500 12h2M18.500 12h2M6 6l1.400 1.400M16.600 16.600 18 18M6 18l1.400-1.400M16.600 7.400 18 6" /></svg>
);
export const MoonIcon = (p: P) => (
  <svg {...base(p)}><path d="M19.500 14.500A7.500 7.500 0 0 1 9.500 4.500a7.500 7.500 0 1 0 10 10Z" /></svg>
);
export const HourglassIcon = (p: P) => (
  <svg {...base(p)}><path d="M7 3.500h10M7 20.500h10M8 3.500c0 4 2.500 5 4 8.500-1.500-3.500-4-4.500-4-8.500Zm8 0c0 4-2.500 5-4 8.500m0 0c-1.500 3.500-4 4.500-4 8.500m4-8.500c1.500 3.500 4 4.500 4 8.500" /></svg>
);
export const CalendarIcon = (p: P) => (
  <svg {...base(p)}><rect x="4" y="5.500" width="16" height="14.500" rx="2" /><path d="M4 10h16M8.500 3.500v4M15.500 3.500v4" /></svg>
);
export const ChipIcon = (p: P) => (
  <svg {...base(p)}><rect x="7" y="7" width="10" height="10" rx="1.500" /><path d="M10 3.500v3.500M14 3.500v3.500M10 17v3.500M14 17v3.500M3.500 10H7M3.500 14H7M17 10h3.500M17 14h3.500" /></svg>
);
