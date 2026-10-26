import type { ActionOutcome } from '$lib/ipc/bindings/ActionOutcome';
import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
import type { SyncAction } from '$lib/ipc/bindings/SyncAction';
import { t, tn, type MessageKey } from '$lib/i18n/i18n.svelte';

/** What the popover says under its rows after an action: it went through, or it needs a word first. */
export interface SyncNote {
  tone: 'success' | 'warning' | 'danger';
  /** The title line. */
  text: string;
  /** A dim line under the title, when there is more to say. */
  detail?: string;
  /** Branch names inside `text`, which the note sets in the code font. */
  names?: string[];
}

/** One run of a note's title: plain words, or a branch name. */
export interface NotePart {
  text: string;
  name: boolean;
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** Cuts `text` into runs so every one of `names` can be set apart from the words around it. */
export function noteParts(text: string, names: string[] = []): NotePart[] {
  const known = names.filter((name) => name !== '').sort((a, b) => b.length - a.length);
  if (known.length === 0) {
    return [{ text, name: false }];
  }

  const pattern = new RegExp(`(${known.map(escapeRegExp).join('|')})`);

  return text
    .split(pattern)
    .filter((part) => part !== '')
    .map((part) => ({ text: part, name: known.includes(part) }));
}

/** The toast title of an action that git refused or that failed on the way. */
export const FAILED_TITLES: Record<SyncAction, MessageKey> = {
  push: 'branch.failed.push',
  publish: 'branch.failed.publish',
  pull: 'branch.failed.pull',
  rebase: 'branch.failed.rebase',
  merge: 'branch.failed.merge',
  abort: 'branch.failed.abort',
  continue: 'branch.failed.continue',
  applyStash: 'branch.failed.applyStash',
  dropStash: 'branch.failed.dropStash',
};

/**
 * The line shown once `action` ended with `outcome`, with `before` the state it started from and `after` the state it left.
 * A stopped rebase or merge says nothing here: the conflict panel takes over from the state.
 */
export function noteFor(action: SyncAction, outcome: ActionOutcome, before: BranchSync | null, after: BranchSync): SyncNote | null {
  if (outcome === 'stopped') {
    return null;
  }

  if (outcome === 'blockedByChanges') {
    return { tone: 'warning', text: t('branch.note.blocked') };
  }

  // The action went through but the stash it took along would not go back cleanly: the files hold conflict markers.
  if (outcome === 'stashKept') {
    return { tone: 'warning', text: t('branch.note.stashKept') };
  }

  if (outcome === 'upToDate') {
    return { tone: 'success', text: t('branch.note.upToDate') };
  }

  const upstream = after.upstream ?? before?.upstream ?? '';
  const main = after.main?.name ?? before?.main?.name ?? '';
  const operation = before?.operation?.kind;

  const pushed = before?.ahead ?? 0;
  const pulled = before?.behind ?? 0;

  const notes: Record<SyncAction, Omit<SyncNote, 'tone'>> = {
    push: {
      text: t('branch.note.pushed', { upstream }),
      names: [upstream],
      detail: pushed > 0 ? tn('branch.note.pushed.detail', pushed) : undefined,
    },
    publish: {
      text: t('branch.note.published', { upstream }),
      names: [upstream],
    },
    pull: {
      text: t('branch.note.pulled', { upstream }),
      names: [upstream],
      detail: pulled > 0 ? tn('branch.note.pulled.detail', pulled) : undefined,
    },
    rebase: { text: t('branch.note.rebased', { main }), names: [main] },
    merge: { text: t('branch.note.merged', { main }), names: [main] },
    abort: {
      text: operation === 'merge' ? t('branch.note.aborted.merge') : t('branch.note.aborted.rebase'),
    },
    continue: {
      text: operation === 'merge' ? t('branch.note.continued.merge') : t('branch.note.continued.rebase'),
    },
    applyStash: { text: t('branch.note.stashApplied') },
    dropStash: { text: t('branch.note.stashDropped') },
  };

  return { tone: 'success', ...notes[action] };
}
