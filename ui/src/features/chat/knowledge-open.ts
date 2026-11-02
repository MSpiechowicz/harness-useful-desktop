import { idFromKey } from '$lib/ipc/knowledge-id';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

/**
 * Opens a document in the Knowledge view, from a card in the chat. The panel of a document reads it by its id, so no filter of the
 * list has to change: it opens whatever the list shows.
 */
export function showDocument(ref: string): void {
  viewState.knowledgeTab = 'docs';
  viewState.knowledgeDocId = idFromKey(ref);
  nav.go('knowledge');
}
