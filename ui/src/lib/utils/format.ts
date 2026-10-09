const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const UNITS: { limit: number; size: number; unit: Intl.RelativeTimeFormatUnit }[] = [
  { limit: HOUR, size: MINUTE, unit: 'minute' },
  { limit: DAY, size: HOUR, unit: 'hour' },
  { limit: 30 * DAY, size: DAY, unit: 'day' },
  { limit: 365 * DAY, size: 30 * DAY, unit: 'month' },
];

/** The UI is English, so numbers and dates follow it and never the language of the operating system. */
export const LOCALE = 'en-US';

const relative = new Intl.RelativeTimeFormat(LOCALE, { numeric: 'auto' });
const COUNT = new Intl.NumberFormat(LOCALE);
const CLOCK = new Intl.DateTimeFormat(LOCALE, { hour: '2-digit', minute: '2-digit' });
const LONG_DATE = new Intl.DateTimeFormat(LOCALE, { year: 'numeric', month: 'long', day: 'numeric' });
const DATE_TIME = new Intl.DateTimeFormat(LOCALE, { dateStyle: 'medium', timeStyle: 'short' });

/** A whole number with grouping: `2,914`. */
export function formatCount(count: number): string {
  return COUNT.format(count);
}

/** The time of day: `02:30 PM`. */
export function formatClock(date: Date): string {
  return CLOCK.format(date);
}

/** A calendar date: `October 8, 2026`. */
export function formatLongDate(date: Date): string {
  return LONG_DATE.format(date);
}

/** A date with its time: `Oct 8, 2026, 6:42 PM`; empty for a value that is not a date. */
export function formatDateTime(value: string | number | Date): string {
  const date = new Date(value);

  return Number.isNaN(date.getTime()) ? '' : DATE_TIME.format(date);
}

/** "just now", "5 minutes ago", "yesterday", "3 months ago" for an RFC 3339 time. */
export function relativeTime(iso: string, now: number = Date.now()): string {
  const time = Date.parse(iso);
  if (Number.isNaN(time)) {
    return '';
  }

  const elapsed = now - time;
  if (elapsed < MINUTE) {
    return 'just now';
  }

  const step = UNITS.find((candidate) => elapsed < candidate.limit);
  if (!step) {
    return relative.format(-Math.floor(elapsed / (365 * DAY)), 'year');
  }

  return relative.format(-Math.floor(elapsed / step.size), step.unit);
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

  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

/** A path that may wrap after each separator, so a narrow column never splits a file name. */
export function breakable(path: string): string {
  return path.replace(/([\\/])/g, '$1\u200b');
}
