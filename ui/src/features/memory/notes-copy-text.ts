import type { MessageKey } from '$lib/i18n/locales/en';
import type { PluralBase } from '$lib/i18n/i18n.svelte';
import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';

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
  toastDone: PluralBase;
  toastStays: PluralBase;
  leftover: PluralBase;
  discardTitle: PluralBase;
  discardBody: MessageKey;
  discardBodyAll: MessageKey;
  discardDone: PluralBase;
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
    toastStays: 'memory.copy.toast.stays',
    leftover: 'memory.copy.leftover',
    discardTitle: 'memory.copy.discard.title',
    discardBody: 'memory.copy.discard.body',
    discardBodyAll: 'memory.copy.discard.bodyAll',
    discardDone: 'memory.copy.discard.done',
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
    toastStays: 'memory.copy.scripts.toast.stays',
    leftover: 'memory.copy.scripts.leftover',
    discardTitle: 'memory.copy.scripts.discard.title',
    discardBody: 'memory.copy.scripts.discard.body',
    discardBodyAll: 'memory.copy.scripts.discard.bodyAll',
    discardDone: 'memory.copy.scripts.discard.done',
    discardFailed: 'memory.copy.scripts.discard.failed',
  },
};
