const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const UNITS: { limit: number; size: number; unit: Intl.RelativeTimeFormatUnit }[] = [
  { limit: HOUR, size: MINUTE, unit: 'minute' },
  { limit: DAY, size: HOUR, unit: 'hour' },
  { limit: 30 * DAY, size: DAY, unit: 'day' },
  { limit: 365 * DAY, size: 30 * DAY, unit: 'month' },
];

const relative = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

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
