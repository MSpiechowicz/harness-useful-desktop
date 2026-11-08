import { t, tn } from '$lib/i18n/i18n.svelte';
import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';
import type { NotesCopyResult } from '$lib/ipc/bindings/NotesCopyResult';

import type { NotesCopyFlow } from './notes-copy-state.svelte';
import { COPY_TEXT, pickText } from './notes-copy-text';

/** The notes now where the copy goes: copied, overwritten or already there. Not the ones deleted here, a failed relation deletes nothing. To this computer only the ones added count. */
export function copiedCount({ counts }: NotesCopyResult, mode: NotesCopyMode = 'stranded'): number {
  return mode === 'toLocal' ? counts.added : counts.added + counts.updated + counts.overwritten + counts.identical;
}

/**
 * The notes that failed, each counted once. To the remote a failed note comes with one failure per relation, all with its local id, and a failure
 * without a local id is no note. To this computer a note is known by the remote id.
 */
export function failedNotes({ failed }: NotesCopyResult, mode: NotesCopyMode = 'stranded'): number {
  return new Set(failed.map((failure) => (mode === 'toLocal' ? failure.teamId : failure.localId)).filter((id) => id !== null)).size;
}

/** The title of the dialog for what it is showing. */
export function copyTitle(flow: NotesCopyFlow): string {
  const text = COPY_TEXT[flow.kind];
  const pick = pickText(flow.kind, flow.mode);

  switch (flow.stage) {
    case 'applying':
      return t(flow.picking ? pick.titleApplying : text.titleApplying);
    case 'result':
      return t('memory.copy.title.done');
    case 'failed':
      return t('memory.copy.title.failed');
    case 'loading':
      return t(flow.picking ? pick.title : text.titleLoading);
    case 'review':
      if (!flow.preview || flow.preview.total === 0) {
        return t('memory.copy.title.none');
      }

      if (flow.picking) {
        return t(pick.title);
      }

      return flow.copyable > 0 ? tn(text.title, flow.copyable) : tn(text.blockedTitle, flow.blocked);
  }
}

/** The line under the title. */
export function copyDescription(flow: NotesCopyFlow): string {
  const text = COPY_TEXT[flow.kind];
  const pick = pickText(flow.kind, flow.mode);

  switch (flow.stage) {
    case 'applying':
      return t('memory.copy.description.applying');
    case 'result':
      return t(flow.picking ? pick.descriptionDone : text.descriptionDone);
    case 'failed':
      return flow.failure ?? '';
    case 'loading':
      return t(text.loadingHint);
    case 'review':
      if (!flow.preview || flow.preview.total === 0) {
        return t(flow.picking ? pick.descriptionNone : text.descriptionNone);
      }

      if (flow.picking) {
        return t(flow.copyable > 0 ? pick.description : pick.descriptionNone);
      }

      return t(flow.copyable > 0 ? text.description : text.blockedHelp);
  }
}
