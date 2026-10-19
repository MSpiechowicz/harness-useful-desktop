import { t, tn } from '$lib/i18n/i18n.svelte';
import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import type { ConfirmRequest } from '../sessions/SessionConfirm.svelte';
import { COPY_TEXT } from './notes-copy-text';

/** Which entries saved on this computer go: the ones named, or everything there is in the scopes that read the remote database (`count` of them were counted). */
export type DiscardTarget = { ids: readonly number[] } | { all: number };

const countOf = (target: DiscardTarget): number => ('all' in target ? target.all : target.ids.length);

/** The list of a kind, which a copy or a discard changes. */
export const listOf = (kind: MemoryCopyKind) => (kind === 'notes' ? noteList : scriptList);

/** The question before notes or scripts saved on this computer are deleted: they are invisible while their scope reads the remote database. */
export function discardRequest(kind: MemoryCopyKind, target: DiscardTarget): ConfirmRequest {
  const text = COPY_TEXT[kind];

  return {
    title: tn(text.discardTitle, countOf(target)),
    description: t('all' in target ? text.discardBodyAll : text.discardBody),
    confirmLabel: t('memory.copy.discard.confirm'),
    cancelLabel: t('memory.deleteAll.cancel'),
  };
}

/** Deletes the entries of a kind from the local file: `all` of them, or those local ids. How many went. */
function discard(kind: MemoryCopyKind, target: DiscardTarget): Promise<number> {
  const ids = 'all' in target ? 'all' : [...target.ids];

  return kind === 'notes' ? ipc.invoke('memory_notes_discard_local', { ids }) : ipc.invoke('memory_scripts_discard_local', { ids });
}

/**
 * Deletes notes or scripts saved on this computer, after `ask` (a confirmation) says yes. `{ ids }` deletes exactly those, as a copy's leftover
 * (one saved after the comparison is not among them). `{ all }` deletes everything there is in the scopes that read the remote database.
 * Says how many went in a toast, and answers whether the call succeeded.
 */
export async function discardLocal(kind: MemoryCopyKind, target: DiscardTarget, ask: (request: ConfirmRequest) => Promise<boolean>): Promise<boolean> {
  if (!(await ask(discardRequest(kind, target)))) {
    return false;
  }

  const text = COPY_TEXT[kind];

  try {
    toasts.push({ title: tn(text.discardDone, await discard(kind, target)), tone: 'success' });
  } catch (error) {
    toasts.push({ title: t(text.discardFailed), description: IpcFailure.from(error).message, tone: 'danger' });

    return false;
  } finally {
    await listOf(kind).refresh({ quiet: true });
  }

  return true;
}
