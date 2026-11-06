import { untrack } from 'svelte';

import { idKey } from '$lib/ipc/knowledge-id';
import { KNOWLEDGE_POLL_MS } from '$lib/stores/live';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { type DocsRequest, docsKey, knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { heldKey, type HeldRequest, knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
import { revalidateProjectScopes } from '$lib/stores/project-scopes.svelte';
import { project } from '$lib/stores/project.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { autoRefresh } from '$lib/utils/auto-refresh';

import { settled } from '../memory/memory-data.svelte';
import { AUTHOR_SORT, PATHS_SORT, USEFUL_SORTS, sortOf } from './knowledge-format';
import { liveIdOf, versionOf } from './review-model';

/** The request the documents table makes under what the view state holds. `usefulKnown` is false for a server that does not count usefulness: it has no such filter or sort. `draftsKnown` is false for one that keeps no drafts, `pathsSortable` for one that cannot sort by paths, `authorKnown` for one that cannot filter or sort by who added a document. */
export function docsRequest(query: string, usefulKnown = true, draftsKnown = true, pathsSortable = true, authorKnown = true): DocsRequest {
  return {
    query,
    scope: viewState.knowledgeScope,
    format: viewState.knowledgeFormat,
    place: viewState.knowledgePlace,
    useful: usefulKnown ? viewState.knowledgeUseful : null,
    status: draftsKnown ? viewState.knowledgeStatus : null,
    author: authorKnown ? viewState.knowledgeAuthor : null,
    sort: sortOf(viewState.knowledgeSort.key, usefulKnown, pathsSortable, authorKnown),
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

  // A server that does not count how useful documents were has no such columns, filter or sort: a choice of them is dropped, back to the default sort.
  const usefulUnsupported = $derived(knowledgeStatus.value?.features.docsUseful === false);

  $effect(() => {
    if (!usefulUnsupported) {
      return;
    }

    if (viewState.knowledgeUseful) {
      viewState.knowledgeUseful = null;
    }

    if (USEFUL_SORTS.some((sort) => sort === viewState.knowledgeSort.key)) {
      viewState.knowledgeSort = { key: 'updated', desc: true };
    }
  });

  // A server that keeps no drafts has no State filter either.
  const draftsUnsupported = $derived(knowledgeStatus.value?.features.docsDrafts === false);

  $effect(() => {
    if (draftsUnsupported && viewState.knowledgeStatus !== null) {
      viewState.knowledgeStatus = null;
    }
  });

  // A server that cannot sort by paths would refuse the whole list.
  const pathsUnsupported = $derived(knowledgeStatus.value?.features.docsSortPaths === false);

  $effect(() => {
    if (pathsUnsupported && viewState.knowledgeSort.key === PATHS_SORT) {
      viewState.knowledgeSort = { key: 'updated', desc: true };
    }
  });

  // Nor can one that does not filter by author keep the Added by filter or sort.
  const authorUnsupported = $derived(knowledgeStatus.value?.features.docsAuthorFilter === false);

  $effect(() => {
    if (!authorUnsupported) {
      return;
    }

    if (viewState.knowledgeAuthor) {
      viewState.knowledgeAuthor = null;
    }

    if (viewState.knowledgeSort.key === AUTHOR_SORT) {
      viewState.knowledgeSort = { key: 'updated', desc: true };
    }
  });

  const documents = $derived<DocsRequest>(docsRequest(docsQuery.value, !usefulUnsupported, !draftsUnsupported, !pathsUnsupported, !authorUnsupported));
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

  // The history, comments and comparison of the open document: of its pending version when it has one. They are read for a server that keeps drafts, and stay read while the full review is open.
  const opened = $derived(knowledgeDocs.detail.data?.document.id === viewState.knowledgeDocId ? knowledgeDocs.detail.data?.document : undefined);
  const version = $derived(knowledgeStatus.value?.features.docsDrafts === true && opened ? versionOf(opened) : null);
  const historyId = $derived(knowledgeStatus.value?.features.docsDrafts === true && opened ? (version?.id ?? opened.id) : null);

  $effect(() => {
    const id = historyId;

    untrack(() => knowledgeReview.history.sync(id === null ? null : idKey(id), null));
  });

  // A live document with a pending update has the rounds of the update under the pending row and its own events under the live row: the History section reads both.
  const liveId = $derived(version && opened ? liveIdOf(opened) : null);

  $effect(() => {
    const id = liveId;

    untrack(() => knowledgeReview.liveHistory.sync(id === null ? null : idKey(id), null));
  });

  $effect(() => {
    const id = version?.id ?? null;
    const key = id === null ? null : idKey(id);

    untrack(() => {
      knowledgeReview.comments.sync(key, null);
      knowledgeReview.compare.sync(key, null);
    });
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
    untrack(() => void Promise.all([knowledgeRemote.load(), knowledgeStatus.load(), knowledgeStats.load()]));
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

      untrack(() => void Promise.all([knowledgeDocs.refresh(), knowledgeHeld.refresh(), knowledgeReview.refresh(), knowledgeStats.load()]));
    }
  });

  // The database tells when it changes, so this is only the backstop: a server that was missing or failing is asked again on the retry schedule. The comparison of the open version is read
  // with the document, so the diff on screen and the revision a verdict is made on never drift apart.
  $effect(() => autoRefresh(() => Promise.all([knowledgeDocs.refresh(), knowledgeHeld.refresh(), knowledgeReview.refresh()]), KNOWLEDGE_POLL_MS, { failing: knowledgeDocs.list.status === 'error' }));

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
