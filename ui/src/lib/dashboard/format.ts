import { t } from '$lib/i18n/i18n.svelte';
import { perLocale } from '$lib/utils/format';

const GROUPED = perLocale((l) => new Intl.NumberFormat(l, { minimumFractionDigits: 2, maximumFractionDigits: 2 }));
/** Exactly `digits` decimals in the language's notation, without grouping: `0.119` / `0,119`. */
const FIXED = [0, 1, 2, 3, 4].map((digits) =>
  perLocale((l) => new Intl.NumberFormat(l, { minimumFractionDigits: digits, maximumFractionDigits: digits, useGrouping: false })),
);
/** Up to one decimal, a trailing `.0` dropped: `17.6`, `2` / `17,6`. */
const UP_TO_ONE = perLocale((l) => new Intl.NumberFormat(l, { maximumFractionDigits: 1, useGrouping: false }));

function fixed(value: number, digits: number): string {
  return FIXED[digits]().format(value);
}
type SpanUnit = 'day' | 'hour' | 'minute';

/** The language's shortest way to write a span of time: `3d`, `2h`, `33m`; `3 T`, `33 Min.` in German. */
const SPANS = perLocale((l) => {
  const unit = (name: SpanUnit) => new Intl.NumberFormat(l, { style: 'unit', unit: name, unitDisplay: 'narrow' });

  return { day: unit('day'), hour: unit('hour'), minute: unit('minute') };
});
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
    return `${sign}$${GROUPED().format(value)}`;
  }

  return `${sign}$${fixed(value, value >= 0.01 ? 3 : 4)}`;
}

/** A token count in compact notation: `842`, `17.6k`, `1.2M`, `3.4B` (`17,6k` in Polish). */
export function formatTokens(count: number): string {
  const value = Math.abs(count);
  const unit = COMPACT_UNITS.find((candidate) => value >= candidate.size);

  if (!unit) {
    return String(Math.round(count));
  }

  const scaled = count / unit.size;
  const text = Math.abs(scaled) >= 100 ? fixed(scaled, 0) : UP_TO_ONE().format(Number(scaled.toFixed(1)));

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

  return `${percent >= 10 ? fixed(percent, 0) : UP_TO_ONE().format(Number(percent.toFixed(1)))}%`;
}

/** A signed relative change as `+23.6%` / `-12.6%`. */
export function formatChange(change: number): string {
  const percent = change * 100;
  const text = fixed(Math.abs(percent), Math.abs(percent) >= 10 ? 0 : 1);

  return `${percent < 0 ? '-' : '+'}${text}%`;
}

/** A count of days, hours or minutes in the language's shortest unit. */
export function formatSpan(value: number, unit: SpanUnit): string {
  return SPANS()[unit].format(value);
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
    return t('ecosystem.time.in', { time: `${formatSpan(days, 'day')} ${formatSpan(hours, 'hour')}` });
  }
  if (hours > 0) {
    return t('ecosystem.time.in', { time: `${formatSpan(hours, 'hour')} ${formatSpan(rest, 'minute')}` });
  }

  return t('ecosystem.time.in', { time: formatSpan(Math.max(rest, 1), 'minute') });
}
