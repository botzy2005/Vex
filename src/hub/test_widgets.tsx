/**
 * TEST HARNESS: NOT PART OF THE SHIPPED APP.
 * Probe widgets that let the UI tests observe the Hub's lifecycle: lazy loading, mount/unmount,
 * the `active` flag, persistence through `useWidgetData`, and a widget that throws.
 */
import type { WidgetDef } from '../shared/types';
import { registerWidget, type WidgetProps } from './registry';
import { GridIcon } from './icons';
import { useWidgetData } from './useWidgetData';
import { useEffect } from 'react';

export interface ProbeLog {
  loads: string[];
  mounts: Record<string, number>;
  unmounts: Record<string, number>;
  /** Last `active` value each probe saw. */
  active: Record<string, boolean>;
}

declare global {
  interface Window {
    __probe?: ProbeLog;
    /** While not false, probe-crash throws when rendered. */
    __hubCrash?: boolean;
  }
}

const log = (): ProbeLog => (window.__probe ??= { loads: [], mounts: {}, unmounts: {}, active: {} });

export const PROBE_IDS = ['probe-1', 'probe-2', 'probe-3', 'probe-4', 'probe-5', 'probe-6'];

export const TEST_WIDGETS: WidgetDef[] = [
  ...PROBE_IDS.map((id, i): WidgetDef => ({
    id,
    title: `Probe ${i + 1}`,
    description: `Test widget ${i + 1}`,
    sizes: ['s', 'm', 't', 'l'],
    defaultSize: 's',
  })),
  { id: 'probe-crash', title: 'Crashy', description: 'Throws when rendered', sizes: ['s', 'm'], defaultSize: 's' },
];

function Probe({ kind, size, active }: WidgetProps) {
  const data = useWidgetData<{ text: string }>(kind, { text: '' });
  useEffect(() => {
    const l = log();
    l.mounts[kind] = (l.mounts[kind] ?? 0) + 1;
    return () => {
      l.unmounts[kind] = (l.unmounts[kind] ?? 0) + 1;
    };
  }, [kind]);
  useEffect(() => {
    log().active[kind] = active;
  }, [kind, active]);
  return (
    <div data-testid="probe" data-kind={kind} data-size={size} data-active={active}>
      <input
        data-testid="probe-input"
        aria-label="Probe text"
        value={data.value.text}
        disabled={!data.loaded}
        onChange={(e) => data.update({ text: e.target.value })}
      />
    </div>
  );
}

function Crash(_: WidgetProps): never {
  if (window.__hubCrash !== false) throw new Error('probe exploded');
  throw new Error('unreachable');
}

function Healthy({ kind }: WidgetProps) {
  return <div data-testid="probe" data-kind={kind}>recovered</div>;
}

export function registerTestWidgets() {
  for (const id of PROBE_IDS) {
    registerWidget(id, {
      icon: GridIcon,
      load: async () => {
        log().loads.push(id);
        return { default: Probe };
      },
    });
  }
  registerWidget('probe-crash', {
    icon: GridIcon,
    load: async () => {
      log().loads.push('probe-crash');
      return { default: window.__hubCrash === false ? Healthy : (Crash as unknown as typeof Probe) };
    },
  });
}
