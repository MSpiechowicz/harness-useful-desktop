import { untrack } from 'svelte';

import type { NoteSort } from '$lib/ipc/bindings/NoteSort';
import type { ScriptSort } from '$lib/ipc/bindings/ScriptSort';
import { noteDetail, noteList, type NotesRequest, notesKey } from '$lib/stores/memoryNotes.svelte';
import { scriptList, type ScriptsRequest, scriptsKey } from '$lib/stores/memoryScripts.svelte';
import { project } from '$lib/stores/project.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { autoRefresh } from '$lib/utils/auto-refresh';

/** The columns the server sorts the notes by: all of them, and a column is named by its key. */
const NOTE_SORTS: readonly NoteSort[] = ['updated', 'title', 'kind', 'scope', 'place'];

/** The columns the server sorts the scripts by: all of them. */
const SCRIPT_SORTS: readonly ScriptSort[] = ['updated', 'name', 'language', 'scope', 'place', 'runs', 'lastRun'];

/** The sort a column key asks for; a key that is not a column (a view state left by an older version) sorts by time. */
function sortOf<T extends string>(allowed: readonly T[], key: string): T {
  return allowed.find((sort) => sort === key) ?? allowed[0];
}

/** How often an open view looks for scripts the agents saved or ran meanwhile (short, so the table feels live). */
const SCRIPTS_REFRESH_MS = 5_000;

/** Notes change less often. */
const NOTES_REFRESH_MS = 15_000;

/** How long a search waits for the next key before it asks the server, so typing a word is one read. */
const SEARCH_DELAY_MS = 250;

/** What a search field held once typing paused. Clearing the field is applied at once. Call it while a component initialises. */
function settled(read: () => string): { readonly value: string } {
  let value = $state(read());

  $effect(() => {
    const typed = read();

    if (typed === untrack(() => value)) {
      return;
    }

    const timer = setTimeout(() => (value = typed), typed === '' ? 0 : SEARCH_DELAY_MS);

    return () => clearTimeout(timer);
  });

  return {
    get value() {
      return value;
    },
  };
}

/**
 * Keeps both lists of the Memory view read for this project, so the section menu can count them, and the one on screen
 * fresh. Call it once while the view initialises: it registers the effects of the calling component.
 */
export function useMemoryData(): void {
  const path = $derived(project.active?.path ?? null);
  const noteQuery = settled(() => viewState.notesQuery);
  const scriptQuery = settled(() => viewState.scriptsQuery);

  // A server is asked for what it answers: the scope next to the kind only while it combines filters (it says so with every page).
  const combinedNotes = $derived(noteList.data?.combinedFilters ?? true);
  const combinedScripts = $derived(scriptList.data?.combinedFilters ?? true);

  const notes = $derived<NotesRequest>({
    query: noteQuery.value,
    scope: combinedNotes ? viewState.notesScope : null,
    kind: viewState.notesKind,
    sort: sortOf(NOTE_SORTS, viewState.notesSort.key),
    order: viewState.notesSort.desc ? 'desc' : 'asc',
    page: viewState.notesPage,
  });

  const scripts = $derived<ScriptsRequest>({
    query: scriptQuery.value,
    scope: combinedScripts ? viewState.scriptsScope : null,
    review: combinedScripts ? viewState.scriptsStatus : null,
    group: combinedScripts ? null : viewState.scriptsGroup,
    sort: sortOf(SCRIPT_SORTS, viewState.scriptsSort.key),
    order: viewState.scriptsSort.desc ? 'desc' : 'asc',
    page: viewState.scriptsPage,
  });

  // Opening the view (or switching project, page, search or sort) reads again; a list already read is quietly refreshed.
  $effect(() => {
    const key = notesKey(notes);
    const where = path;

    // The store reads its own state while loading; only the request and the project may re-run this.
    untrack(() => noteList.revalidate(key, where));
  });

  $effect(() => {
    const key = scriptsKey(scripts);
    const where = path;

    untrack(() => scriptList.revalidate(key, where));
  });

  $effect(() => {
    if (viewState.memorySection === 'scripts') {
      return autoRefresh(() => void scriptList.refresh({ quiet: true }), SCRIPTS_REFRESH_MS);
    }

    return autoRefresh(() => {
      void noteList.refresh({ quiet: true });
      void noteDetail.refresh({ quiet: true });
    }, NOTES_REFRESH_MS);
  });
}
