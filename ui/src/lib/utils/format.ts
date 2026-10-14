import { i18n, t } from '$lib/i18n/i18n.svelte';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const UNITS: { limit: number; size: number; unit: Intl.RelativeTimeFormatUnit }[] = [
  { limit: HOUR, size: MINUTE, unit: 'minute' },
  { limit: DAY, size: HOUR, unit: 'hour' },
  { limit: 30 * DAY, size: DAY, unit: 'day' },
  { limit: 365 * DAY, size: 30 * DAY, unit: 'month' },
];

/** The locale of the language on screen, so numbers and dates follow it and never the language of the operating system. */
export function locale(): string {
  return i18n.locale;
}

/** One formatter per locale, made on first use. */
export function perLocale<T>(make: (locale: string) => T): () => T {
  const made = new Map<string, T>();

  return () => {
    const current = locale();
    let formatter = made.get(current);

    if (!formatter) {
      formatter = make(current);
      made.set(current, formatter);
    }

    return formatter;
  };
}

const relative = perLocale((l) => new Intl.RelativeTimeFormat(l, { numeric: 'auto' }));
const COUNT = perLocale((l) => new Intl.NumberFormat(l));
const CLOCK = perLocale((l) => new Intl.DateTimeFormat(l, { hour: '2-digit', minute: '2-digit' }));
const LONG_DATE = perLocale((l) => new Intl.DateTimeFormat(l, { year: 'numeric', month: 'long', day: 'numeric' }));
const DATE_TIME = perLocale((l) => new Intl.DateTimeFormat(l, { dateStyle: 'medium', timeStyle: 'short' }));
const ONE_DECIMAL = perLocale((l) => new Intl.NumberFormat(l, { minimumFractionDigits: 1, maximumFractionDigits: 1 }));
const WHOLE = perLocale((l) => new Intl.NumberFormat(l, { maximumFractionDigits: 0 }));
const COST = perLocale((l) => new Intl.NumberFormat(l, { minimumFractionDigits: 4, maximumFractionDigits: 4 }));

/** A whole number with grouping: `2,914`. */
export function formatCount(count: number): string {
  return COUNT().format(count);
}

/** A per-call cost in USD with four decimals, in the language's notation: `$0.0123`, `$0,0123`. */
export function formatCost(usd: number): string {
  return `$${COST().format(usd)}`;
}

/** The time of day: `02:30 PM`. */
export function formatClock(date: Date): string {
  return CLOCK().format(date);
}

/** A calendar date: `October 8, 2026`. */
export function formatLongDate(date: Date): string {
  return LONG_DATE().format(date);
}

/** A date with its time: `Oct 8, 2026, 6:42 PM`; empty for a value that is not a date. */
export function formatDateTime(value: string | number | Date): string {
  const date = new Date(value);

  return Number.isNaN(date.getTime()) ? '' : DATE_TIME().format(date);
}

const SHORT_UNIT = perLocale((l) => (unit: Intl.NumberFormatOptions['unit'], value: number) => new Intl.NumberFormat(l, { style: 'unit', unit, unitDisplay: 'narrow' }).format(value));

/** A list row's age in as few characters as the language allows: "6h", "2d", "3w", "5mo". */
export function compactAge(iso: string, now: number = Date.now()): string {
  const time = Date.parse(iso);
  if (Number.isNaN(time)) {
    return '';
  }

  const elapsed = Math.max(0, now - time);
  if (elapsed < MINUTE) {
    return t('common.justNow');
  }

  const steps: { limit: number; size: number; unit: string }[] = [
    { limit: HOUR, size: MINUTE, unit: 'minute' },
    { limit: DAY, size: HOUR, unit: 'hour' },
    { limit: 7 * DAY, size: DAY, unit: 'day' },
    { limit: 30 * DAY, size: 7 * DAY, unit: 'week' },
    { limit: 365 * DAY, size: 30 * DAY, unit: 'month' },
  ];
  const step = steps.find((candidate) => elapsed < candidate.limit);

  return step ? SHORT_UNIT()(step.unit, Math.floor(elapsed / step.size)) : SHORT_UNIT()('year', Math.floor(elapsed / (365 * DAY)));
}

/** "just now", "5 minutes ago", "yesterday", "3 months ago" for an RFC 3339 time. */
export function relativeTime(iso: string, now: number = Date.now()): string {
  const time = Date.parse(iso);
  if (Number.isNaN(time)) {
    return '';
  }

  const elapsed = now - time;
  if (elapsed < MINUTE) {
    return t('common.justNow');
  }

  const step = UNITS.find((candidate) => elapsed < candidate.limit);
  if (!step) {
    return relative().format(-Math.floor(elapsed / (365 * DAY)), 'year');
  }

  return relative().format(-Math.floor(elapsed / step.size), step.unit);
}

/** "812 B", "48.2 KB", "1.9 MB". */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = 0;

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }

  return `${(value < 10 ? ONE_DECIMAL() : WHOLE()).format(value)} ${units[unit]}`;
}

/** A path that may wrap after each separator, so a narrow column never splits a file name. */
export function breakable(path: string): string {
  return path.replace(/([\\/])/g, '$1\u200b');
}
