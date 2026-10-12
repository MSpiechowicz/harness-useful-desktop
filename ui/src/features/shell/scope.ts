import { tn } from '$lib/i18n/i18n.svelte';

/**
 * "12 sessions · 3 on this branch": the state line under a view's title. The harness and the project are already
 * named by the switchers in the top bar, so the header only adds what is specific to the view.
 */
export function scopeLabel(...parts: (string | null | undefined)[]): string {
  return parts.filter((part): part is string => Boolean(part)).join(' · ');
}

const NOUNS = {
  session: 'shell.count.session',
  file: 'shell.count.file',
  'changed file': 'shell.count.changedFile',
} as const;

/** "3 sessions" / "1 session", with the plural forms of the language on screen. */
export function count(amount: number, noun: keyof typeof NOUNS): string {
  return tn(NOUNS[noun], amount);
}
