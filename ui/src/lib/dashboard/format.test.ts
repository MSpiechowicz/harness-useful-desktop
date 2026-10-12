import { afterEach, describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';

import { formatChange, formatPercent, formatTokens, formatUsd, timeUntil } from './format';

describe('dashboard number formats', () => {
  it('writes costs with grouping and keeps small amounts readable', () => {
    expect(formatUsd(1234.5)).toBe('$1,234.50');
    expect(formatUsd(142.9071)).toBe('$142.91');
    expect(formatUsd(0.1187)).toBe('$0.119');
    expect(formatUsd(0.0084)).toBe('$0.0084');
    expect(formatUsd(0)).toBe('$0.00');
  });

  it('writes token counts in compact notation', () => {
    expect(formatTokens(842)).toBe('842');
    expect(formatTokens(17_600)).toBe('17.6k');
    expect(formatTokens(1_200_000)).toBe('1.2M');
    expect(formatTokens(34_178_800)).toBe('34.2M');
    expect(formatTokens(127_507_600)).toBe('128M');
    expect(formatTokens(2_000_000)).toBe('2M');
  });

  it('signs changes and trims percentages', () => {
    expect(formatChange(0.236)).toBe('+24%');
    expect(formatChange(-0.126)).toBe('-13%');
    expect(formatChange(0.031)).toBe('+3.1%');
    expect(formatPercent(42.5)).toBe('43%');
    expect(formatPercent(4.25)).toBe('4.3%');
    expect(formatPercent(100)).toBe('100%');
  });

  it('counts down to a reset and says nothing once it has passed', () => {
    const now = Date.parse('2026-10-09T12:00:00Z');

    expect(timeUntil('2026-10-09T14:33:00Z', now)).toBe('in 2h 33m');
    expect(timeUntil('2026-10-12T16:00:00Z', now)).toBe('in 3d 4h');
    expect(timeUntil('2026-10-09T11:00:00Z', now)).toBeNull();
  });
});

describe('dashboard times in another language', () => {
  afterEach(() => void i18n.load('en'));

  it('writes the countdown in the language on screen', async () => {
    await i18n.load('pl');

    expect(timeUntil('2026-10-09T14:33:00Z', Date.parse('2026-10-09T12:00:00Z'))).toBe('za 2 h 33 min');
  });

  it('writes numbers with the language’s decimal separator, so one page never mixes `,` and `.`', async () => {
    await i18n.load('pl');

    expect(formatUsd(0.119)).toBe('$0,119');
    expect(formatTokens(17_600)).toBe('17,6k');
    expect(formatTokens(34_200_000)).toBe('34,2M');
    expect(formatPercent(7.5)).toBe('7,5%');
    expect(formatChange(-0.034)).toBe('-3,4%');
  });
});
