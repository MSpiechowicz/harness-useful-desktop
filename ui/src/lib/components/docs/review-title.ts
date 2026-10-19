import type { MessageKey } from '$lib/i18n/locales/en';

/** The headline a review gives, by what happened: who changed the document, and whether it is new to the user. */
export interface ReviewTitleKeys {
  added: MessageKey;
  addedOutside: MessageKey;
  changed: MessageKey;
  changedOutside: MessageKey;
  teamAdded: MessageKey;
  teamChanged: MessageKey;
}

/**
 * The headline of a review. A document that was never approved is new to the user (`added`), one with an approved version has
 * been changed since. An agent wrote it or somebody did outside the app, and a team document says whose it is.
 */
export function reviewTitleKey(keys: ReviewTitleKeys, state: { team: boolean; added: boolean; byAgent: boolean }): MessageKey {
  if (state.team) {
    return state.added ? keys.teamAdded : keys.teamChanged;
  }

  if (state.added) {
    return state.byAgent ? keys.added : keys.addedOutside;
  }

  return state.byAgent ? keys.changed : keys.changedOutside;
}
