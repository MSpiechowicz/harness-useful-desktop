import type { BreakdownRow, TokenBreakdown } from '$lib/dashboard/models';

/** Where the input tokens of a range came from: the cache, a cache fill, or fresh. */
export interface CacheMix {
  read: number;
  write: number;
  fresh: number;
  /** All input-side tokens, the whole the hit rate is a share of. */
  total: number;
  /** 0..1, or null when there was no input at all. */
  hitRate: number | null;
}

/** The dashboard's own hit rate wins; without one it is the cache reads over all input-side tokens. */
export function cacheMix(tokens: TokenBreakdown, reported: number | null): CacheMix {
  const read = Math.max(tokens.cacheRead, 0);
  const write = Math.max(tokens.cacheWrite, 0);
  const fresh = Math.max(tokens.input, 0);
  const total = read + write + fresh;
  const rate = reported ?? (total > 0 ? read / total : null);

  return { read, write, fresh, total, hitRate: rate === null ? null : Math.min(Math.max(rate, 0), 1) };
}

export interface CacheRank {
  key: string;
  /** 0..1 */
  hitRate: number;
}

/** The rows that lean on the cache most traffic-wise: heaviest first, rows without a rate left out. */
export function cacheRanking(rows: readonly BreakdownRow[], limit: number): CacheRank[] {
  return rows
    .filter((row): row is BreakdownRow & { cacheHitRate: number } => row.cacheHitRate !== null)
    .sort((a, b) => b.tokens - a.tokens)
    .slice(0, limit)
    .map((row) => ({ key: row.key, hitRate: Math.min(Math.max(row.cacheHitRate, 0), 1) }));
}
