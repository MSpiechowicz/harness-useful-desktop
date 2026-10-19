import { untrack } from 'svelte';

import type { NoteSort } from '$lib/ipc/bindings/NoteSort';
import type { ScriptSort } from '$lib/ipc/bindings/ScriptSort';
import { MEMORY_NOTES_POLL_MS, MEMORY_SCRIPTS_POLL_MS } from '$lib/stores/live';
import { noteDetail, noteList, type NotesRequest, notesKey } from '$lib/stores/memoryNotes.svelte';
import { scriptDetail, scriptList, type ScriptsRequest, scriptsKey } from '$lib/stores/memoryScripts.svelte';
import { project } from '$lib/stores/project.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { autoRefresh } from '$lib/utils/auto-refresh';

import { noteEditor, scriptEditor } from './editor-state.svelte';
import { notesImport } from './notes-import-state.svelte';

/** The columns the server sorts the notes by: all of them, and a column is named by its key. */
const NOTE_SORTS: readonly NoteSort[] = ['updated', 'title', 'kind', 'scope', 'place'];

/** The columns the server sorts the scripts by: all of them. */
const SCRIPT_SORTS: readonly ScriptSort[] = ['updated', 'name', 'language', 'scope', 'place', 'runs', 'lastRun'];

/** The sort a column key asks for; a key that is not a column (a view state left by an older version) sorts by time. */
function sortOf<T extends string>(allowed: readonly T[], key: string): T {
  return allowed.find((sort) => sort === key) ?? allowed[0];
}

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
 * Keeps the list of the Notes view read for this project: the page the search, the filters, the sort and the page ask for, and the open note.
 * Call it once while the view initialises: it registers the effects of the calling component.
 */
export function useNotesData(): void {
  const path = $derived(project.active?.path ?? null);
  const query = settled(() => viewState.notesQuery);

  // A server is asked for what it answers: the scope next to the kind only while it combines filters (it says so with every page).
  const combined = $derived(noteList.data?.combinedFilters ?? true);

  const request = $derived<NotesRequest>({
    query: query.value,
    scope: combined ? viewState.notesScope : null,
    kind: viewState.notesKind,
    sort: sortOf(NOTE_SORTS, viewState.notesSort.key),
    order: viewState.notesSort.desc ? 'desc' : 'asc',
    page: viewState.notesPage,
  });

  // The editor and the import review belong to the project on screen and to this view: neither outlives a switch of project or leaving the view.
  $effect(() => {
    void path;

    return () => {
      noteEditor.reset();
      notesImport.reset();
    };
  });

  // Opening the view (or switching project, page, search or sort) reads again; a list already read is quietly refreshed.
  $effect(() => {
    const key = notesKey(request);
    const where = path;

    // The store reads its own state while loading; only the request and the project may re-run this.
    untrack(() => noteList.revalidate(key, where));
  });

  $effect(() => {
    // A list that failed to load is read again on the retry schedule, a little later each time, instead of at the poll's pace.
    return autoRefresh(() => Promise.all([noteList.refresh({ quiet: true }), noteDetail.refresh({ quiet: true })]), MEMORY_NOTES_POLL_MS, {
      failing: noteList.status === 'error',
    });
  });
}

/** The same for the Scripts view: its list, read for this project and kept fresh. Call it once while the view initialises. */
export function useScriptsData(): void {
  const path = $derived(project.active?.path ?? null);
  const query = settled(() => viewState.scriptsQuery);

  const combined = $derived(scriptList.data?.combinedFilters ?? true);

  const request = $derived<ScriptsRequest>({
    query: query.value,
    scope: combined ? viewState.scriptsScope : null,
    review: combined ? viewState.scriptsStatus : null,
    group: combined ? null : viewState.scriptsGroup,
    sort: sortOf(SCRIPT_SORTS, viewState.scriptsSort.key),
    order: viewState.scriptsSort.desc ? 'desc' : 'asc',
    page: viewState.scriptsPage,
  });

  // The editor belongs to the project on screen and to this view: it does not outlive a switch of project or leaving the view.
  $effect(() => {
    void path;

    return scriptEditor.reset.bind(scriptEditor);
  });

  $effect(() => {
    const key = scriptsKey(request);
    const where = path;

    untrack(() => scriptList.revalidate(key, where));
  });

  // The open script is read again with every look at the list, so what an agent changed in it shows whether or not its row is on this page.
  $effect(() => autoRefresh(() => Promise.all([scriptList.refresh({ quiet: true }), scriptDetail.refresh({ quiet: true })]), MEMORY_SCRIPTS_POLL_MS, { failing: scriptList.status === 'error' }));
}
