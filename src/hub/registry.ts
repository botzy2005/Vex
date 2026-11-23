import type { ComponentType, SVGProps } from 'react';
import type { WidgetSize } from '../shared/types';
import { CalendarIcon, CompassIcon, GlobeIcon, HourglassIcon, NoteIcon, TimerIcon } from './icons';

/** What the Hub hands every widget. */
export interface WidgetProps {
  kind: string;
  size: WidgetSize;
  /** False while the Hub is hidden or minimised: stop polling and animating. */
  active: boolean;
}

export interface WidgetEntry {
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  /** Dynamic import, so a widget's code is only fetched when its card is first shown. */
  load: () => Promise<{ default: ComponentType<WidgetProps> }>;
}

/**
 * UI side of the catalog (`src/shared/widgets.json` says what exists; this says how to draw it).
 * Adding a widget = one catalog entry + one line here + a component.
 */
const entries = new Map<string, WidgetEntry>([
  ['welcome', { icon: CompassIcon, load: () => import('./widgets/Welcome') }],
  ['notes', { icon: NoteIcon, load: () => import('./widgets/notes/Notes') }],
  ['worldclock', { icon: GlobeIcon, load: () => import('./widgets/worldclock/WorldClock') }],
  ['countdown', { icon: HourglassIcon, load: () => import('./widgets/countdown/Countdown') }],
  ['calendar', { icon: CalendarIcon, load: () => import('./widgets/calendar/Calendar') }],
  ['pomodoro', { icon: TimerIcon, load: () => import('./widgets/pomodoro/Pomodoro') }],
]);

export const entryOf = (kind: string): WidgetEntry | undefined => entries.get(kind);
export const registerWidget = (kind: string, entry: WidgetEntry): void => void entries.set(kind, entry);
