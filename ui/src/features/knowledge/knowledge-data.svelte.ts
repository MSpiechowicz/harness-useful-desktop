import { untrack } from 'svelte';

import { idKey } from '$lib/ipc/knowledge-id';
import { KNOWLEDGE_POLL_MS } from '$lib/stores/live';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { type DocsRequest, docsKey, knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { heldKey, type HeldRequest, knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
import { revalidateProjectScopes } from '$lib/stores/project-scopes.svelte';
import { project } from '$lib/stores/project.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { autoRefresh } from '$lib/utils/auto-refresh';

import { settled } from '../memory/memory-data.svelte';
import { sortOf } from './knowledge-format';

/** The request the documents table makes under what the view state holds. */
export function docsRequest(query: string): DocsRequest {
  return {
    query,
    scope: viewState.knowledgeScope,
    format: viewState.knowledgeFormat,
    place: viewState.knowledgePlace,
    sort: sortOf(viewState.knowledgeSort.key),
    order: viewState.knowledgeSort.desc ? 'desc' : 'asc',
    page: viewState.knowledgePage,
  };
}

/**
 * Keeps what the Knowledge view shows read: the page the search, the filters, the sort and the page ask for, the open document and proposal,
 * and the repositories of the Scope filter. The database tells when it changed (`knowledgeChanges`), so nothing is polled and no list needs a Refresh button.
 * Call it once while the view initialises: it registers the effects of the calling component.
 */
export function useKnowledgeData(): void {
  const docsQuery = settled(() => viewState.knowledgeQuery);
  const heldQuery = settled(() => viewState.heldQuery);
  const active = $derived(project.active?.path ?? null);

  const documents = $derived<DocsRequest>(docsRequest(docsQuery.value));
  const proposals = $derived<HeldRequest>({ query: heldQuery.value, page: viewState.heldPage, scope: viewState.heldScope, sort: viewState.heldSort });

  // Opening the view (or a change of the search, the filters, the sort or the page) reads again; a list already read is quietly refreshed.
  $effect(() => {
    const key = docsKey(documents);

    untrack(() => knowledgeDocs.list.revalidate(key, null));
  });

  $effect(() => {
    const key = heldKey(proposals);

    untrack(() => knowledgeHeld.list.revalidate(key, null));
  });

  $effect(() => {
    const id = viewState.knowledgeDocId;

    untrack(() => knowledgeDocs.detail.sync(id === null ? null : idKey(id), null));
  });

  $effect(() => {
    const id = viewState.heldId;

    untrack(() => knowledgeHeld.detail.sync(id, null));
  });

  $effect(() => {
    untrack(() => knowledgeDocs.repos.revalidate('repos', null));
  });

  // Where documents are kept, and whether the server can narrow a list to one place, decide whether the Kept in filter is offered.
  $effect(() => {
    untrack(() => void Promise.all([knowledgeRemote.load(), knowledgeStatus.load()]));
  });

  // The repositories of the Scope filter and the editors are those of the open project and the recent ones.
  $effect(() => {
    void active;
    void project.recents;

    untrack(() => revalidateProjectScopes());
  });

  // The backend reads the counts every few seconds only while this view is open.
  $effect(() => {
    knowledgeChanges.watch(true);

    return () => knowledgeChanges.watch(false);
  });

  // The database changed (an agent added a document, a proposal came in, an import went on): everything on screen is read again.
  // The remote database coming back or going away changes what the lists hold even when the generation stays.
  const readState = () => `${knowledgeChanges.pulse?.generation ?? ''}:${knowledgeChanges.pulse?.offline ?? ''}`;
  let seen = untrack(readState);

  $effect(() => {
    const state = readState();

    if (state !== seen) {
      seen = state;

      untrack(() => void Promise.all([knowledgeDocs.refresh(), knowledgeHeld.refresh()]));
    }
  });

  // The database tells when it changes, so this is only the backstop: a server that was missing or failing is asked again on the retry schedule.
  $effect(() => autoRefresh(() => Promise.all([knowledgeDocs.refresh(), knowledgeHeld.refresh()]), KNOWLEDGE_POLL_MS, { failing: knowledgeDocs.list.status === 'error' }));

  // A search starts at its first page.
  let searched = [viewState.knowledgeQuery, viewState.heldQuery];

  $effect(() => {
    const now = [viewState.knowledgeQuery, viewState.heldQuery];

    if (now[0] !== searched[0]) {
      viewState.knowledgePage = 0;
    }

    if (now[1] !== searched[1]) {
      viewState.heldPage = 0;
    }

    searched = now;
  });
}
