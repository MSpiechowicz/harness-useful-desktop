import { describe, expect, it } from 'vitest';

import type { BreakdownRow, TokenBreakdown } from '$lib/dashboard/models';

import { cacheMix, cacheRanking } from './cache';

const tokens = (over: Partial<TokenBreakdown>): TokenBreakdown => ({ total: 0, input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0, ...over });
const row = (key: string, tokens: number, cacheHitRate: number | null): BreakdownRow => ({ key, costUsd: 0, share: 0, tokens, sessions: 0, cacheHitRate, estimatedPrice: false });

describe('cacheMix', () => {
  it('splits the input side and trusts the reported rate', () => {
    const mix = cacheMix(tokens({ input: 182_400, cacheRead: 3_912_000, cacheWrite: 241_800, output: 96_350 }), 0.902);
    expect(mix).toMatchObject({ read: 3_912_000, write: 241_800, fresh: 182_400, total: 4_336_200, hitRate: 0.902 });
  });

  it('derives the rate from the tokens when none is reported', () => {
    expect(cacheMix(tokens({ input: 100, cacheRead: 300 }), null).hitRate).toBe(0.75);
  });

  it('has no rate when there was no input', () => {
    expect(cacheMix(tokens({ output: 50 }), null)).toMatchObject({ total: 0, hitRate: null });
  });

  it('keeps a rate inside 0..1', () => {
    expect(cacheMix(tokens({ input: 1 }), 1.4).hitRate).toBe(1);
  });
});

describe('cacheRanking', () => {
  it('lists the heaviest rows first, skips unrated ones and honors the limit', () => {
    const ranked = cacheRanking([row('a', 10, 0.5), row('b', 90, 0.9), row('c', 50, null), row('d', 70, 0.2)], 2);
    expect(ranked).toEqual([
      { key: 'b', hitRate: 0.9 },
      { key: 'd', hitRate: 0.2 },
    ]);
  });
});
