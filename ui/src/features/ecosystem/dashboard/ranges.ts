import { t } from '$lib/i18n/i18n.svelte';
import type { PanelRange } from '$lib/stores/dashboard.svelte';
import { ownEntry } from '$lib/utils/own-entry';

/** How each range reads in a sentence. */
export const RANGE_PHRASE: Record<PanelRange, string> = {
  get today() {
    return t('ecosystem.phrase.today');
  },
  get '7d'() {
    return t('ecosystem.phrase.7d');
  },
  get '30d'() {
    return t('ecosystem.phrase.30d');
  },
};

/** The phrase for the range the data covers (not the one just chosen, while that still loads). */
export function phraseOf(covered: string | null | undefined, chosen: PanelRange): string {
  return ownEntry(RANGE_PHRASE, covered ?? chosen) ?? RANGE_PHRASE[chosen];
}
