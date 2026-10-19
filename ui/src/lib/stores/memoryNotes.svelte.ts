import { t } from '$lib/i18n/i18n.svelte';
import type { MemoryNotesPage } from '$lib/ipc/bindings/MemoryNotesPage';
import type { NoteDetail } from '$lib/ipc/bindings/NoteDetail';
import type { NoteDraft } from '$lib/ipc/bindings/NoteDraft';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { NoteSaved } from '$lib/ipc/bindings/NoteSaved';
import type { NoteSort } from '$lib/ipc/bindings/NoteSort';
import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';
import type { SortOrder } from '$lib/ipc/bindings/SortOrder';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';

import { hideEntry, memoryItemGone, saveFailureText } from './memoryScripts.svelte';
import { toasts } from './toasts.svelte';
import { viewState } from './view-state.svelte';
import { ViewLoader } from './view-loader.svelte';

/** How many notes one page of the table holds. */
export const NOTES_PAGE_SIZE = 50;

/** What the table asks the server for: one page of the notes that answer the search, the scope and the kind, in this order. */
export interface NotesRequest {
  query: string;
  scope: ScriptScope | null;
  kind: NoteKind | null;
  sort: NoteSort;
  order: SortOrder;
  page: number;
  /** Only the team notes hidden on this computer. */
  hidden: boolean;
}

/** The request as the loader's key: the loader fetches again whenever any of it changes, and drops answers to older ones. */
export function notesKey(request: NotesRequest): string {
  return JSON.stringify(request);
}

/** One page of the notes this project sees (its repository's and the global ones), with the count of every kind and scope. */
export const noteList = new ViewLoader<MemoryNotesPage, string>((key, project) => {
  const { query, scope, kind, sort, order, page, hidden } = JSON.parse(key) as NotesRequest;
  const search = query.trim();

  return ipc.invoke('memory_notes_list', {
    project,
    offset: page * NOTES_PAGE_SIZE,
    limit: NOTES_PAGE_SIZE,
    query: search === '' ? undefined : search,
    scope: scope ?? undefined,
    kind: kind ?? undefined,
    sort,
    order,
    hidden: hidden || undefined,
  });
});

/** The key the open note is read under: a hidden note is only found when asked for as hidden. */
export function noteDetailKey(id: number, hidden: boolean): string {
  return hidden ? `${id}:hidden` : String(id);
}

/** The note on screen, body and links included. The list carries no bodies, so this is read when one is chosen. */
export const noteDetail = new ViewLoader<NoteDetail, string>(
  (key, project) => {
    const [id, hidden] = key.split(':');

    return ipc.invoke('memory_note_get', { id: Number(id), project, hidden: hidden === undefined ? undefined : true });
  },
  { goneIsFinal: memoryItemGone },
);

/** Hides a remote note on this computer, or shows it again (`hidden: false`). See `hideEntry`. */
export function hideNote(id: number, title: string, project: string | null, hidden: boolean): Promise<boolean> {
  return hideEntry({
    id,
    title,
    hidden,
    send: () => ipc.invoke('memory_note_hide', { id, project, hidden }),
    failed: 'memory.hidden.toast.failed',
    unhideFailed: 'memory.hidden.toast.unhideFailed',
    leave: () => {
      if (viewState.noteId === id) {
        viewState.noteId = null;
      }
    },
    refresh: () => noteList.refresh(),
    undo: () => void hideNote(id, title, project, false),
  });
}

/** Deletes a note once the user confirms, then re-reads the page so the row is gone (or back, if the call failed). */
export async function deleteNote(id: number, project: string | null, confirm: () => Promise<boolean>): Promise<boolean> {
  if (!(await confirm())) {
    return false;
  }

  try {
    await ipc.invoke('memory_note_delete', { id, project });
  } catch (error) {
    toasts.push({ title: t('memory.notes.delete.failed'), description: saveFailureText(IpcFailure.from(error)), tone: 'danger' });
    await noteList.refresh();

    return false;
  }

  if (viewState.noteId === id) {
    viewState.noteId = null;
  }
  await noteList.refresh();

  return true;
}

/**
 * Writes a note, then re-reads the list and the open note so what is on screen is what was saved. With `replaceId` the note with that id is rewritten
 * (an edit, or a replace of the note that had the title), without it a note is created. `exists` means a repository note already has the title and nothing was written.
 * A failure is said in a toast, and answers null.
 */
export async function saveNote(project: string | null, draft: NoteDraft, replaceId?: number): Promise<NoteSaved | null> {
  let saved: NoteSaved;

  try {
    saved = await ipc.invoke('memory_note_save', replaceId === undefined ? { project, draft } : { project, draft, replaceId });
  } catch (error) {
    toasts.push({ title: t('memory.editor.note.toast.failed'), description: saveFailureText(IpcFailure.from(error)), tone: 'danger' });

    return null;
  }

  if (saved.outcome !== 'exists') {
    await Promise.all([noteList.refresh({ quiet: true }), noteDetail.refresh({ quiet: true })]);
  }

  return saved;
}
