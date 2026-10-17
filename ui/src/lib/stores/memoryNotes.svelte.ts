import { t } from '$lib/i18n/i18n.svelte';
import type { MemoryNotesPage } from '$lib/ipc/bindings/MemoryNotesPage';
import type { NoteDetail } from '$lib/ipc/bindings/NoteDetail';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { NoteSort } from '$lib/ipc/bindings/NoteSort';
import type { SortOrder } from '$lib/ipc/bindings/SortOrder';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';

import { toasts } from './toasts.svelte';
import { viewState } from './view-state.svelte';
import { ViewLoader } from './view-loader.svelte';

/** How many notes one page of the table holds. */
export const NOTES_PAGE_SIZE = 50;

/** What the table asks the server for: one page of the notes that answer the search and the kind, in this order. */
export interface NotesRequest {
  query: string;
  kind: NoteKind | null;
  sort: NoteSort;
  order: SortOrder;
  page: number;
}

/** The request as the loader's key: the loader fetches again whenever any of it changes, and drops answers to older ones. */
export function notesKey(request: NotesRequest): string {
  return JSON.stringify(request);
}

/** One page of the notes this project sees (its repository's and the global ones), with the count of every kind. */
export const noteList = new ViewLoader<MemoryNotesPage, string>((key, project) => {
  const { query, kind, sort, order, page } = JSON.parse(key) as NotesRequest;
  const search = query.trim();

  return ipc.invoke('memory_notes_list', {
    project,
    offset: page * NOTES_PAGE_SIZE,
    limit: NOTES_PAGE_SIZE,
    query: search === '' ? undefined : search,
    kind: kind ?? undefined,
    sort,
    order,
  });
});

/** The note on screen, body and links included. The list carries no bodies, so this is read when one is chosen. */
export const noteDetail = new ViewLoader<NoteDetail, string>((id, project) => ipc.invoke('memory_note_get', { id: Number(id), project }));

/** Deletes a note once the user confirms, then re-reads the page so the row is gone (or back, if the call failed). */
export async function deleteNote(id: number, project: string | null, confirm: () => Promise<boolean>): Promise<boolean> {
  if (!(await confirm())) {
    return false;
  }

  try {
    await ipc.invoke('memory_note_delete', { id, project });
  } catch (error) {
    toasts.push({ title: t('memory.notes.delete.failed'), description: IpcFailure.from(error).message, tone: 'danger' });
    await noteList.refresh();

    return false;
  }

  if (viewState.noteId === id) {
    viewState.noteId = null;
  }
  await noteList.refresh();

  return true;
}
