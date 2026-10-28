import { i18n } from '$lib/i18n/i18n.svelte';

/** How far into the run a step happened: `0:22`, `1:12:05`. Empty when either time is unknown. */
export function stepClock(at: number | null, startedAt: number | null): string {
  if (at === null || startedAt === null) {
    return '';
  }

  const seconds = Math.max(0, Math.round((at - startedAt) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = String(seconds % 60).padStart(2, '0');

  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${rest}` : `${minutes}:${rest}`;
}

/** The time of day a moment happened at, in the language's own way: `14:02`, `2:02 PM`. */
export function timeOfDay(ms: number): string {
  return new Intl.DateTimeFormat(i18n.locale, { hour: '2-digit', minute: '2-digit' }).format(new Date(ms));
}
