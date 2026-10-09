import { afterEach, describe, expect, it, vi } from 'vitest';

import { simulatePolishOs } from '../../../tests/locale';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.resetModules();
});

/** The formatting modules, loaded as the app loads them on a system whose language is not English. */
async function onPolishSystem() {
  vi.resetModules();
  simulatePolishOs();

  return { format: await import('./format'), copy: await import('../../features/updates/copy') };
}

describe('number and date formatting', () => {
  it('the simulated system really formats differently', () => {
    simulatePolishOs();

    expect((17612).toLocaleString()).not.toBe('17,612');
  });

  it('groups counts the English way whatever the system language is', async () => {
    const { format } = await onPolishSystem();

    expect(format.formatCount(17_612)).toBe('17,612');
    expect(format.formatCount(1_234_567)).toBe('1,234,567');
  });

  it('writes dates and times in English whatever the system language is', async () => {
    const { format, copy } = await onPolishSystem();
    const day = new Date(2026, 9, 8, 18, 42);

    expect(format.formatLongDate(day)).toBe('October 8, 2026');
    expect(format.formatClock(day)).toBe('06:42 PM');
    expect(format.formatDateTime(day)).toBe('Oct 8, 2026, 6:42 PM');
    expect(copy.formatReleaseDate('2026-10-08T12:00:00Z')).toBe('October 8, 2026');
    expect(copy.statusLine({ kind: 'upToDate', checkedAt: day })).toContain('06:42 PM');
  });

  it('shows nothing for a date it cannot read', async () => {
    const { format } = await onPolishSystem();

    expect(format.formatDateTime('not a date')).toBe('');
  });
});
