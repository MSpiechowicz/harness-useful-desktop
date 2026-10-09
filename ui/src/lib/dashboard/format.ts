import { LOCALE } from '$lib/utils/format';

const GROUPED = new Intl.NumberFormat(LOCALE, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const COMPACT_UNITS = [
  { size: 1e9, suffix: 'B' },
  { size: 1e6, suffix: 'M' },
  { size: 1e3, suffix: 'k' },
] as const;

/**
 * A cost in USD: `$1,234.56`, with more decimals below $1 so small amounts do not collapse
 * to `$0.00` (`$0.119`, `$0.0084`).
 */
export function formatUsd(amount: number): string {
  const sign = amount < 0 ? '-' : '';
  const value = Math.abs(amount);

  if (value >= 1 || value === 0) {
    return `${sign}$${GROUPED.format(value)}`;
  }

  return `${sign}$${value.toFixed(value >= 0.01 ? 3 : 4)}`;
}

/** A token count in compact notation: `842`, `17.6k`, `1.2M`, `3.4B`. */
export function formatTokens(count: number): string {
  const value = Math.abs(count);
  const unit = COMPACT_UNITS.find((candidate) => value >= candidate.size);

  if (!unit) {
    return String(Math.round(count));
  }

  const scaled = count / unit.size;
  const text = Math.abs(scaled) >= 100 ? scaled.toFixed(0) : scaled.toFixed(1).replace(/\.0$/, '');

  return `${text}${unit.suffix}`;
}

/** A 0..1 share as `61.7%` (one decimal below 10%, none from 100%). */
export function formatShare(share: number): string {
  return formatPercent(share * 100);
}

/** A percentage that is already 0..100: `42.5%`, `100%`. */
export function formatPercent(percent: number): string {
  if (percent >= 99.95) {
    return `${Math.round(percent)}%`;
  }

  return `${percent.toFixed(percent >= 10 ? 0 : 1).replace(/\.0$/, '')}%`;
}

/** A signed relative change as `+23.6%` / `-12.6%`. */
export function formatChange(change: number): string {
  const percent = change * 100;
  const text = Math.abs(percent) >= 10 ? Math.abs(percent).toFixed(0) : Math.abs(percent).toFixed(1);

  return `${percent < 0 ? '-' : '+'}${text}%`;
}

/** "in 2h 33m" / "in 3d 4h" for a future ISO time; `null` when it has passed or cannot be read. */
export function timeUntil(iso: string, now: number = Date.now()): string | null {
  const time = Date.parse(iso);
  if (Number.isNaN(time) || time <= now) {
    return null;
  }

  const minutes = Math.round((time - now) / 60_000);
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  const rest = minutes % 60;

  if (days > 0) {
    return `in ${days}d ${hours}h`;
  }
  if (hours > 0) {
    return `in ${hours}h ${rest}m`;
  }

  return `in ${Math.max(rest, 1)}m`;
}
