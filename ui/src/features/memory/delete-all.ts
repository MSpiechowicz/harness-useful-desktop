import { t, tn } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { type MemorySection, viewState } from '$lib/stores/view-state.svelte';

/** The most entries one list call returns, and so the most whose place can be told before the question is asked. */
const PROBE_LIMIT = 200;

/** What "delete all" takes away in one section, whatever search or kind the list on screen is narrowed to. */
export interface DeleteScope {
  total: number;
  /** How many of them are kept in the team database. */
  team: number;
  /** Whether `team` counts every entry. A longer list shows only its first page, so some more may be the team's. */
  exact: boolean;
}

/** Reads the whole section as the project sees it: one page, as the list does, but with no search and no filter. Null when it cannot be read. */
export async function readScope(section: MemorySection, project: string | null): Promise<DeleteScope | null> {
  const window = { project, offset: 0, limit: PROBE_LIMIT };

  try {
    const page = section === 'notes' ? await ipc.invoke('memory_notes_list', window) : await ipc.invoke('memory_scripts_list', window);

    if (page.availability.kind !== 'available') {
      return null;
    }

    const places = 'notes' in page ? page.notes.map((note) => note.place) : page.scripts.map((script) => script.place);

    return { total: page.total, team: places.filter((place) => place === 'team').length, exact: page.total <= places.length };
  } catch {
    return null;
  }
}

/** Deletes everything of one section, then lets go of the open row, reads the list again and says how many went. Rejects with the server's reason. */
export async function deleteAll(section: MemorySection, project: string | null): Promise<void> {
  const args = { project: project ?? undefined };
  const result = section === 'notes' ? await ipc.invoke('memory_notes_delete_all', args) : await ipc.invoke('memory_scripts_delete_all', args);

  if (section === 'notes') {
    viewState.noteId = null;
    viewState.notesPage = 0;
  } else {
    viewState.scriptId = null;
    viewState.scriptsPage = 0;
  }

  const lost = result.team ?? 0;

  toasts.push({
    title: tn(section === 'notes' ? 'memory.deleteAll.notes.done' : 'memory.deleteAll.scripts.done', result.deleted),
    description: lost > 0 ? tn('memory.deleteAll.done.team', lost) : undefined,
    tone: 'success',
  });

  await (section === 'notes' ? noteList : scriptList).refresh();
}

/** The line of the confirmation about the team database; empty when nothing of it is the team's. */
export function teamLine(scope: DeleteScope): string | undefined {
  if (scope.team > 0 && scope.exact) {
    return tn('memory.deleteAll.team', scope.team);
  }

  return scope.exact ? undefined : t('memory.deleteAll.team.maybe');
}
