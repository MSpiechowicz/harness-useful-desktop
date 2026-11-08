import type { MessageKey } from '$lib/i18n/locales/en';
import type { PluralBase } from '$lib/i18n/i18n.svelte';
import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';
import type { NotesCopyRow } from '$lib/ipc/bindings/NotesCopyRow';

/** The words of the copy flow (banner, dialog, deleting what is left) that name what is copied. The rest of the flow reads the same for notes and scripts. */
export interface CopyText {
  bannerText: PluralBase;
  title: PluralBase;
  titleLoading: MessageKey;
  titleApplying: MessageKey;
  description: MessageKey;
  descriptionNone: MessageKey;
  descriptionDone: MessageKey;
  loadingHint: MessageKey;
  statHelp: MessageKey;
  conflictsTitle: MessageKey;
  conflictsHelp: MessageKey;
  blockedTitle: PluralBase;
  blockedHelp: MessageKey;
  blockedTableLabel: MessageKey;
  tableLabel: MessageKey;
  columnTitle: MessageKey;
  groupGlobal: MessageKey;
  failedHelp: MessageKey;
  toastDone: MessageKey;
  toastDoneBody: PluralBase;
  toastStays: PluralBase;
  leftover: PluralBase;
  discardTitle: PluralBase;
  discardBody: MessageKey;
  discardBodyAll: MessageKey;
  discardDone: MessageKey;
  discardDoneBody: PluralBase;
  discardFailed: MessageKey;
}

/** One map per kind: the notes keep their keys, the scripts have their own for every string that mentions notes. */
export const COPY_TEXT: Record<MemoryCopyKind, CopyText> = {
  notes: {
    bannerText: 'memory.copy.banner.text',
    title: 'memory.copy.title',
    titleLoading: 'memory.copy.title.loading',
    titleApplying: 'memory.copy.title.applying',
    description: 'memory.copy.description',
    descriptionNone: 'memory.copy.description.none',
    descriptionDone: 'memory.copy.description.done',
    loadingHint: 'memory.copy.loading.hint',
    statHelp: 'memory.copy.stat.help',
    conflictsTitle: 'memory.copy.conflicts.title',
    conflictsHelp: 'memory.copy.conflicts.help',
    blockedTitle: 'memory.copy.blocked.title',
    blockedHelp: 'memory.copy.blocked.help',
    blockedTableLabel: 'memory.copy.blocked.table.label',
    tableLabel: 'memory.copy.table.label',
    columnTitle: 'memory.copy.column.title',
    groupGlobal: 'memory.copy.group.global',
    failedHelp: 'memory.copy.result.failedHelp',
    toastDone: 'memory.copy.toast.done',
    toastDoneBody: 'memory.copy.toast.doneBody',
    toastStays: 'memory.copy.toast.stays',
    leftover: 'memory.copy.leftover',
    discardTitle: 'memory.copy.discard.title',
    discardBody: 'memory.copy.discard.body',
    discardBodyAll: 'memory.copy.discard.bodyAll',
    discardDone: 'memory.copy.discard.done',
    discardDoneBody: 'memory.copy.discard.doneBody',
    discardFailed: 'memory.copy.discard.failed',
  },
  scripts: {
    bannerText: 'memory.copy.scripts.banner.text',
    title: 'memory.copy.scripts.title',
    titleLoading: 'memory.copy.scripts.title.loading',
    titleApplying: 'memory.copy.scripts.title.applying',
    description: 'memory.copy.scripts.description',
    descriptionNone: 'memory.copy.scripts.description.none',
    descriptionDone: 'memory.copy.scripts.description.done',
    loadingHint: 'memory.copy.scripts.loading.hint',
    statHelp: 'memory.copy.scripts.stat.help',
    conflictsTitle: 'memory.copy.scripts.conflicts.title',
    conflictsHelp: 'memory.copy.scripts.conflicts.help',
    blockedTitle: 'memory.copy.scripts.blocked.title',
    blockedHelp: 'memory.copy.scripts.blocked.help',
    blockedTableLabel: 'memory.copy.scripts.blocked.table.label',
    tableLabel: 'memory.copy.scripts.table.label',
    columnTitle: 'memory.copy.scripts.column.title',
    groupGlobal: 'memory.copy.scripts.group.global',
    failedHelp: 'memory.copy.scripts.result.failedHelp',
    toastDone: 'memory.copy.scripts.toast.done',
    toastDoneBody: 'memory.copy.scripts.toast.doneBody',
    toastStays: 'memory.copy.scripts.toast.stays',
    leftover: 'memory.copy.scripts.leftover',
    discardTitle: 'memory.copy.scripts.discard.title',
    discardBody: 'memory.copy.scripts.discard.body',
    discardBodyAll: 'memory.copy.scripts.discard.bodyAll',
    discardDone: 'memory.copy.scripts.discard.done',
    discardDoneBody: 'memory.copy.scripts.discard.doneBody',
    discardFailed: 'memory.copy.scripts.discard.failed',
  },
};

/** What a copy where the user picks the entries says differently from the stranded one. Nothing is ticked at first, and Copy waits for a tick. */
export interface PickText {
  title: MessageKey;
  titleApplying: MessageKey;
  description: MessageKey;
  descriptionNone: MessageKey;
  descriptionDone: MessageKey;
  /** The heading of the entries that can be picked, also the name of their table. */
  pickTitle: MessageKey;
  blockedHelp: MessageKey;
  failedHelp: MessageKey;
}

/** The words of a copy to this computer, per kind: the remote's entries stay as they are and nothing is removed. The scripts say they need approval before they run. */
export const PULL_TEXT: Record<MemoryCopyKind, PickText> = {
  notes: {
    title: 'memory.sync.pull.title.notes',
    titleApplying: 'memory.sync.pull.titleApplying.notes',
    description: 'memory.sync.pull.description.notes',
    descriptionNone: 'memory.sync.pull.descriptionNone.notes',
    descriptionDone: 'memory.sync.pull.descriptionDone.notes',
    pickTitle: 'memory.sync.pull.pickTitle.notes',
    blockedHelp: 'memory.sync.pull.blockedHelp.notes',
    failedHelp: 'memory.sync.pull.failedHelp.notes',
  },
  scripts: {
    title: 'memory.sync.pull.title.scripts',
    titleApplying: 'memory.sync.pull.titleApplying.scripts',
    description: 'memory.sync.pull.description.scripts',
    descriptionNone: 'memory.sync.pull.descriptionNone.scripts',
    descriptionDone: 'memory.sync.pull.descriptionDone.scripts',
    pickTitle: 'memory.sync.pull.pickTitle.scripts',
    blockedHelp: 'memory.sync.pull.blockedHelp.scripts',
    failedHelp: 'memory.sync.pull.failedHelp.scripts',
  },
};

/** The words of a copy to the remote, per kind: what arrived is removed from this computer, so the user reads it before pressing Copy and not only after. */
const TO_REMOTE_TEXT: Record<MemoryCopyKind, PickText> = {
  notes: {
    title: 'memory.sync.toRemote.title.notes',
    titleApplying: COPY_TEXT.notes.titleApplying,
    description: 'memory.sync.toRemote.description.notes',
    descriptionNone: 'memory.sync.toRemote.none.notes',
    descriptionDone: COPY_TEXT.notes.descriptionDone,
    pickTitle: 'memory.sync.toRemote.pickTitle.notes',
    blockedHelp: COPY_TEXT.notes.blockedHelp,
    failedHelp: COPY_TEXT.notes.failedHelp,
  },
  scripts: {
    title: 'memory.sync.toRemote.title.scripts',
    titleApplying: COPY_TEXT.scripts.titleApplying,
    description: 'memory.sync.toRemote.description.scripts',
    descriptionNone: 'memory.sync.toRemote.none.scripts',
    descriptionDone: COPY_TEXT.scripts.descriptionDone,
    pickTitle: 'memory.sync.toRemote.pickTitle.scripts',
    blockedHelp: COPY_TEXT.scripts.blockedHelp,
    failedHelp: COPY_TEXT.scripts.failedHelp,
  },
};

/** The words of a copy of `kind` going `mode`, where the user picks the entries (to the remote or to this computer). */
export function pickText(kind: MemoryCopyKind, mode: NotesCopyMode): PickText {
  return mode === 'toLocal' ? PULL_TEXT[kind] : TO_REMOTE_TEXT[kind];
}

/** What a row of a picking table says: what ticking it does, or why it cannot be ticked. A failed row says its own reason elsewhere. */
export function rowStatusKey(row: NotesCopyRow, mode: NotesCopyMode): MessageKey {
  if (mode === 'toLocal') {
    return row.action === 'identical' ? 'memory.sync.status.identical' : row.action === 'localDiffers' ? 'memory.sync.status.localDiffers' : 'memory.sync.status.copyable';
  }

  if (row.conflict) {
    return 'memory.sync.status.remoteConflict';
  }

  switch (row.action) {
    case 'identical':
      return 'memory.sync.status.remoteSame';
    case 'teamNewer':
      return 'memory.sync.status.remoteNewer';
    case 'update':
      return 'memory.sync.status.remoteUpdate';
    default:
      return 'memory.sync.status.remoteAdd';
  }
}
