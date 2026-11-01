import type { MemoryEmbedderState } from '$lib/ipc/bindings/MemoryEmbedderState';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

import { EmbedderStore } from './embedder.svelte';
import { knowledgeRemote, type KnowledgeRemoteStore } from './knowledgeRemote.svelte';
import { prefs, type PrefsStore } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';
import { vectorSearchPref } from './vector-search-pref';

/**
 * Vector search of Harness Knowledge, as the Knowledge section of the settings shows it. The embedder's state follows
 * `knowledge_embedder_watch`. Whether the installed server has an embedder comes from its `features` line (`embedderSupported`, read with
 * the remote settings), so nothing shows before that is known.
 */
export class KnowledgeSearchStore extends EmbedderStore<MemoryEmbedderState> {
  constructor(
    transport: Transport,
    notifications: ToastsStore,
    appPrefs: PrefsStore,
    private readonly remote: KnowledgeRemoteStore,
  ) {
    super(
      {
        watch: (onEvent) => transport.invoke('knowledge_embedder_watch', { onEvent }),
        setVectorSearch: (enabled) => transport.invoke('knowledge_set_vector_search', { enabled }),
        removeModel: () => transport.invoke('knowledge_embedder_remove_model', undefined),
      },
      vectorSearchPref(appPrefs, 'knowledgeVectorSearch'),
      notifications,
    );
  }

  /** Whether the installed Harness Knowledge runs vector search. Unknown counts as no: nothing is shown before it is known. */
  get vectorSupported(): boolean {
    return this.remote.value?.embedderSupported === true;
  }
}

export const knowledgeSearch = new KnowledgeSearchStore(ipc, toasts, prefs, knowledgeRemote);
