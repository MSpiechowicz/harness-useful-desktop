import type { PanelRange } from '$lib/stores/dashboard.svelte';

/** How each range reads in a sentence. */
export const RANGE_PHRASE: Record<PanelRange, string> = {
  today: 'today',
  '7d': 'the last 7 days',
  '30d': 'the last 30 days',
};

/** The phrase for the range the data covers (not the one just chosen, while that still loads). */
export function phraseOf(covered: string | null | undefined, chosen: PanelRange): string {
  return RANGE_PHRASE[(covered ?? chosen) as PanelRange] ?? RANGE_PHRASE[chosen];
}
