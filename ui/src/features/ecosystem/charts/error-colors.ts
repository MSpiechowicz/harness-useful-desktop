import { theme } from '$lib/stores/theme.svelte';

interface Pair {
  light: string;
  dark: string;
}

/**
 * Failure causes as the Harness Dashboard's Friction page paints errors: each cause keeps one color in every range,
 * the commonest ones in the status reds and ambers (critical, serious, warning), the rest in the dashboard's general
 * families. Never green, which would read as success, and never a color a harness owns in the same chart.
 */
const CAUSE_COLORS: Record<string, Pair> = {
  exit_code: { light: '#d03b3b', dark: '#d03b3b' },
  not_found: { light: '#ec835a', dark: '#ec835a' },
  edit_mismatch: { light: '#fab219', dark: '#fab219' },
  timeout: { light: '#009cbc', dark: '#00b4d7' },
  rejected: { light: '#8279ee', dark: '#9791ff' },
  permission: { light: '#d65895', dark: '#ef6fab' },
  stale_read: { light: '#9e8d00', dark: '#b6a300' },
  bad_input: { light: '#92125a', dark: '#ffaacd' },
};

/** The dashboard's "Other": a neutral gray outside every family. */
const OTHER: Pair = { light: '#b5b3ab', dark: '#5f5e59' };

/** The color of failure cause `cause`; unknown causes and the folded rest are gray. */
export function causeColor(cause: string | null): string {
  return (cause ? (CAUSE_COLORS[cause] ?? OTHER) : OTHER)[theme.resolved];
}
