import { type FactRow, momentFact } from '$lib/components/facts';
import type { Column } from '$lib/components/table';
import { type MessageKey, t, tn } from '$lib/i18n/i18n.svelte';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { NoteKindCount } from '$lib/ipc/bindings/NoteKindCount';
import type { NoteSummary } from '$lib/ipc/bindings/NoteSummary';

import { authorFacts, PLACE_LABELS } from './script-format';

/** The kinds in the order the chips show them: what was decided and fixed first, loose notes last. */
export const NOTE_KINDS: readonly NoteKind[] = ['decision', 'fix', 'problem', 'outcome', 'optimisation', 'location', 'note'];

/** The plural name of a kind, on a chip: "Decisions". */
export const KIND_GROUP_LABELS = {
  decision: 'memory.notes.group.decision',
  fix: 'memory.notes.group.fix',
  problem: 'memory.notes.group.problem',
  outcome: 'memory.notes.group.outcome',
  optimisation: 'memory.notes.group.optimisation',
  location: 'memory.notes.group.location',
  note: 'memory.notes.group.note',
} as const satisfies Record<NoteKind, MessageKey>;

/** The name of one kind, in a row or under a title: "Decision". */
export const KIND_LABELS = {
  decision: 'memory.notes.kind.decision',
  fix: 'memory.notes.kind.fix',
  problem: 'memory.notes.kind.problem',
  outcome: 'memory.notes.kind.outcome',
  optimisation: 'memory.notes.kind.optimisation',
  location: 'memory.notes.kind.location',
  note: 'memory.notes.kind.note',
} as const satisfies Record<NoteKind, MessageKey>;

/** Every column sorts, by the server. They are all text or dates, so they read from the left. A time starts with the newest. */
export function noteColumns(): Column[] {
  return [
    { key: 'title', label: t('memory.notes.column.title'), sortable: true },
    { key: 'kind', label: t('memory.notes.column.kind'), width: '9rem', sortable: true },
    { key: 'scope', label: t('memory.notes.column.scope'), width: '12rem', from: 'md', sortable: true },
    { key: 'place', label: t('memory.notes.column.place'), width: '8rem', from: 'lg', sortable: true },
    { key: 'updated', label: t('memory.notes.column.updated'), width: '8rem', sortable: true, descFirst: true, from: 'sm' },
  ];
}

/** How many notes there are of one kind under the current search; kinds the server did not count have none. */
export function kindCount(kinds: readonly NoteKindCount[], kind: NoteKind): number {
  return kinds.find((entry) => entry.kind === kind)?.count ?? 0;
}

/** All notes under the current search: the kinds partition them. */
export function totalCount(kinds: readonly NoteKindCount[]): number {
  return kinds.reduce((sum, entry) => sum + entry.count, 0);
}

/** A repository note's repository for people: the key of a folder without a remote reads as its folder name. */
function repoName(repo: string): string {
  if (!repo.startsWith('path:')) {
    return repo;
  }

  const folder = repo.slice('path:'.length).split(/[\\/]/).filter(Boolean).at(-1);

  return folder ?? repo;
}

/** Where a note applies: everywhere (global), its repository, or a neutral label for a repository note that names none. */
export function noteScopeText(note: Pick<NoteSummary, 'scope' | 'repo'>): string {
  if (note.scope !== 'repo') {
    return t('memory.scripts.row.global');
  }

  return note.repo?.trim() ? repoName(note.repo) : t('memory.scripts.row.thisRepo');
}

/** The Details card of a note: where it applies and is kept, who wrote it, when, and how often it was rewritten and recalled. */
export function noteFacts(note: NoteSummary): FactRow[] {
  return [
    { label: t('memory.detail.scope'), value: noteScopeText(note), title: note.repo ?? undefined },
    { label: t('memory.detail.place'), value: t(PLACE_LABELS[note.place]) },
    ...authorFacts(note),
    momentFact(t('memory.detail.created'), note.createdAt),
    momentFact(t('memory.detail.updated'), note.updatedAt),
    { label: t('memory.notes.detail.revision'), value: String(note.revision) },
    { label: t('memory.notes.detail.recalled'), value: note.hits === 0 ? t('memory.notes.detail.neverRecalled') : tn('memory.notes.detail.times', note.hits) },
  ];
}
