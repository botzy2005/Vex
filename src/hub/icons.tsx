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
