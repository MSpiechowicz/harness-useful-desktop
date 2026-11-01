import { untrack } from 'svelte';

import type { VectorSearchSwitch } from './embedder.svelte';
import type { PrefsStore } from './prefs.svelte';

/** The pref of the app that is the vector search switch of a Harness family member, and the flag that says a change of it is in flight. */
const FIELDS = {
  vectorSearch: 'vectorSearchPending',
  knowledgeVectorSearch: 'knowledgeVectorSearchPending',
} as const;

/** The stored pref of the app as the switch of one member's vector search (on by default). */
export function vectorSearchPref(appPrefs: PrefsStore, field: keyof typeof FIELDS): VectorSearchSwitch {
  return {
    get on() {
      return appPrefs.value?.[field] ?? true;
    },
    stored: () => untrack(() => appPrefs.value?.[field]),
    hold: (pending) => {
      appPrefs[FIELDS[field]] = pending;
    },
    store: (on) => {
      if (appPrefs.value) {
        appPrefs.value = { ...appPrefs.value, [field]: on };
      }
    },
    resync: () => appPrefs.syncServerManaged(),
  };
}
