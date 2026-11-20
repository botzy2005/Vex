import type { SVGProps } from 'react';

const base = (p: SVGProps<SVGSVGElement>) => ({
  width: 18, height: 18, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 1.8,
  strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const, 'aria-hidden': true, ...p,
});

export const CropIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M7 3v13a1 1 0 0 0 1 1h13M3 7h13a1 1 0 0 1 1 1v13" /></svg>;
export const ArrowIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M5 19 19 5M9 5h10v10" /></svg>;
export const LineIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M5 19 19 5" /></svg>;
export const RectIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><rect x="4" y="6" width="16" height="12" rx="2" /></svg>;
export const CircleIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><ellipse cx="12" cy="12" rx="8" ry="6.5" /></svg>;
export const TextIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M5 7V5h14v2M12 5v14M9 19h6" /></svg>;
export const HighlightIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="m4 20 3-1 11-11-2-2L5 17l-1 3ZM14 6l4 4M12 21h8" /></svg>;
export const BlurIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M12 3c3 4 6 7 6 11a6 6 0 0 1-12 0c0-4 3-7 6-11Z" /></svg>;
export const PixelateIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><rect x="4" y="4" width="7" height="7" /><rect x="13" y="13" width="7" height="7" /><path d="M13 4h7v7h-7zM4 13h7v7H4z" opacity=".45" /></svg>;
export const RotateLeftIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><rect x="10" y="11" width="10" height="9" rx="2" /><path d="M16 7h-6a4 4 0 0 0-4 4M8 9l-2 2-2-2" /></svg>;
export const RotateRightIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><rect x="4" y="11" width="10" height="9" rx="2" /><path d="M8 7h6a4 4 0 0 1 4 4M16 9l2 2 2-2" /></svg>;
export const ResizeIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M14 4h6v6M10 20H4v-6M20 4l-7 7M4 20l7-7" /></svg>;
export const UndoIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M9 7 4 12l5 5M4 12h10a6 6 0 0 1 6 6" /></svg>;
export const RedoIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="m15 7 5 5-5 5M20 12H10a6 6 0 0 0-6 6" /></svg>;
export const CopyIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></svg>;
export const SaveIcon = (p: SVGProps<SVGSVGElement>) => <svg {...base(p)}><path d="M12 4v11m0 0-4-4m4 4 4-4M5 19h14" /></svg>;
