import { type PluralBase, t, tn } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import { ipc } from '$lib/ipc/client';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

/** The two lists "delete all" acts on: the Notes view and the Scripts view each have one. */
export type MemorySection = 'notes' | 'scripts';

/** What each section says about the remote entries that "delete all" leaves where they are: they can only be hidden. */
export const STAY_TEXT: Record<MemorySection, { stay: PluralBase; maybe: MessageKey; onlyTeam: PluralBase }> = {
  notes: { stay: 'memory.deleteAll.stay', maybe: 'memory.deleteAll.stay.maybe', onlyTeam: 'memory.deleteAll.onlyTeam' },
  scripts: { stay: 'memory.deleteAll.scripts.stay', maybe: 'memory.deleteAll.scripts.stay.maybe', onlyTeam: 'memory.deleteAll.scripts.onlyTeam' },
};

/** The most entries one list call returns, and so the most whose place can be told before the question is asked. */
const PROBE_LIMIT = 200;

/** What "delete all" takes away in one section, whatever search or kind the list on screen is narrowed to. */
export interface DeleteScope {
  total: number;
  /** How many of them are kept in the team database and go with the rest. */
  team: number;
  /** How many remote entries are read-only here (a scope that saves on this computer): "delete all" leaves them where they are. */
  stay: number;
  /** Whether `team` and `stay` count every entry. A longer list shows only its first page, so some more may be the team's. */
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

    const entries = 'notes' in page ? page.notes : page.scripts;
    const stays = (entry: (typeof entries)[number]): boolean => 'readOnly' in entry && entry.readOnly;
    const stay = entries.filter(stays).length;
    const team = entries.filter((entry) => entry.place === 'team' && !stays(entry)).length;
    const exact = page.total <= entries.length;

    // The remote entries that stay are not deleted: the question counts what goes.
    return { total: exact ? page.total - stay : page.total, team, stay, exact };
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
  const kept = result.kept ?? 0;

  toasts.push({
    title: tn(section === 'notes' ? 'memory.deleteAll.notes.done' : 'memory.deleteAll.scripts.done', result.deleted),
    description: [lost > 0 ? tn('memory.deleteAll.done.team', lost) : '', kept > 0 ? tn(STAY_TEXT[section].stay, kept) : ''].filter(Boolean).join(' ') || undefined,
    tone: 'success',
  });

  await (section === 'notes' ? noteList : scriptList).refresh();
}

/** The lines of the confirmation about the remote database: what the team loses, and the remote entries that stay. Empty when neither applies. */
export function teamLine(section: MemorySection, scope: DeleteScope): string | undefined {
  const lines: string[] = [];

  if (scope.team > 0 && scope.exact) {
    lines.push(tn('memory.deleteAll.team', scope.team));
  } else if (!scope.exact) {
    lines.push(t('memory.deleteAll.team.maybe'));
  }

  if (scope.stay > 0) {
    lines.push(scope.exact ? tn(STAY_TEXT[section].stay, scope.stay) : t(STAY_TEXT[section].maybe));
  }

  return lines.length > 0 ? lines.join(' ') : undefined;
}
