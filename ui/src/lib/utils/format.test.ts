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

describe('formatting follows the language on screen', () => {
  it('groups counts with the Polish separator once the UI is Polish', async () => {
    const { format } = await onPolishSystem();
    const { i18n: fresh } = await import('$lib/i18n/i18n.svelte');

    await fresh.load('pl');

    expect(format.formatCount(17_612)).toBe('17\u00a0612');
    expect(format.formatLongDate(new Date(2026, 9, 8))).toBe('8 października 2026');
    expect(format.formatClock(new Date(2026, 9, 8, 18, 42))).toBe('18:42');
  });

  it('writes relative times and "just now" in the language', async () => {
    const { format } = await onPolishSystem();
    const { i18n: fresh } = await import('$lib/i18n/i18n.svelte');
    const now = Date.parse('2026-10-08T12:00:00Z');

    expect(format.relativeTime('2026-10-08T11:59:30Z', now)).toBe('just now');
    expect(format.relativeTime('2026-10-08T11:55:00Z', now)).toBe('5 minutes ago');

    await fresh.load('de');

    expect(format.relativeTime('2026-10-08T11:59:30Z', now)).toBe('gerade eben');
    expect(format.relativeTime('2026-10-08T11:55:00Z', now)).toBe('vor 5 Minuten');
  });

  it('writes sizes with the decimal mark of the language', async () => {
    const { format } = await onPolishSystem();
    const { i18n: fresh } = await import('$lib/i18n/i18n.svelte');

    expect(format.formatBytes(812)).toBe('812 B');
    expect(format.formatBytes(1_990_000)).toBe('1.9 MB');

    await fresh.load('de');

    expect(format.formatBytes(1_990_000)).toBe('1,9 MB');
  });
});
