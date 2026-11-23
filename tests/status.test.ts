import { describe, expect, it } from 'vitest';
import { pickStatus, statusText } from '../src/island/status';

const s = (kind: string, priority: number, detail?: string) => ({ kind, label: kind, detail, priority });

describe('pickStatus', () => {
  it('picks the highest priority of 1 or more', () => {
    expect(pickStatus([s('a', 0), s('b', 1), s('c', 2)], true)?.kind).toBe('c');
    expect(pickStatus([s('a', 1), s('b', 1)], true)?.kind).toBe('a');
  });
  it('ignores ambient (0) statuses and the disabled setting', () => {
    expect(pickStatus([s('a', 0)], true)).toBeNull();
    expect(pickStatus([s('a', 3)], false)).toBeNull();
    expect(pickStatus([], true)).toBeNull();
  });
  it('joins label and detail', () => {
    expect(statusText(s('Focus', 1, 'ends 14:32'))).toBe('Focus · ends 14:32');
    expect(statusText(s('Focus', 1))).toBe('Focus');
  });
});
