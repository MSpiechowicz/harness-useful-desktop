import type { Column } from '$lib/components/table';
import { i18n, type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { NotesImportPlan } from '$lib/ipc/bindings/NotesImportPlan';
import type { NotesImportPreview } from '$lib/ipc/bindings/NotesImportPreview';
import type { NotesImportProblem } from '$lib/ipc/bindings/NotesImportProblem';
import type { NotesImportResult } from '$lib/ipc/bindings/NotesImportResult';
import type { NotesImportRow } from '$lib/ipc/bindings/NotesImportRow';

/** How many rows of the review one page shows; a file with fewer is shown whole. */
export const IMPORT_PAGE_SIZE = 50;

/** Why a picked file could not be read as notes, and the line of a CSV file it broke at. */
export type NotesImportRefusal = Extract<NotesImportPreview, { kind: 'refused' }>;

/** A row without a problem can be imported. */
export function importable(row: NotesImportRow): boolean {
  return row.problems.length === 0;
}

/** The rows ticked at first: every row without a problem that does not repeat the title of an earlier row of the file. */
export function defaultPicks(rows: readonly NotesImportRow[]): Set<number> {
  return new Set(rows.filter((row) => importable(row) && row.duplicateOf === null).map((row) => row.index));
}

/** Every row that can be imported, or none. */
export function allPicks(rows: readonly NotesImportRow[], on: boolean): Set<number> {
  return on ? new Set(rows.filter(importable).map((row) => row.index)) : new Set();
}

/** The ticked rows in the file's order, which is the order the import saves them in. */
export function pickedIndexes(rows: readonly NotesImportRow[], picks: ReadonlySet<number>): number[] {
  return rows.filter((row) => importable(row) && picks.has(row.index)).map((row) => row.index);
}

/** The page of rows on screen. */
export function pageOfRows(rows: readonly NotesImportRow[], page: number): NotesImportRow[] {
  return rows.slice(page * IMPORT_PAGE_SIZE, (page + 1) * IMPORT_PAGE_SIZE);
}

/** Whether any row takes the dialog's scope because the file gives it none. */
export function needsDefaultScope(rows: readonly NotesImportRow[], picks: ReadonlySet<number>): boolean {
  return rows.some((row) => importable(row) && picks.has(row.index) && row.scope === null);
}

/** The title of a row of the plan, for the result's lists. */
export function titleOfRow(plan: NotesImportPlan | null, index: number): string {
  return plan?.rows.find((row) => row.index === index)?.title ?? '';
}

/** The columns of the review. The first holds the checkbox and has no name worth showing. */
export function reviewColumns(): Column[] {
  return [
    { key: 'pick', label: t('memory.import.column.pick'), width: '3.5rem' },
    { key: 'number', label: t('memory.import.column.number'), width: '3.5rem' },
    { key: 'title', label: t('memory.import.column.title') },
    { key: 'kind', label: t('memory.import.column.kind'), width: '8rem' },
    { key: 'tags', label: t('memory.import.column.tags'), width: '10rem', from: 'md' },
    { key: 'scope', label: t('memory.import.column.scope'), width: '8rem' },
    { key: 'body', label: t('memory.import.column.body'), width: '16rem', from: 'lg' },
  ];
}

const PROBLEM_LABELS = {
  missing: 'memory.import.problem.missing',
  tooLong: 'memory.import.problem.tooLong',
  multiLine: 'memory.import.problem.multiLine',
  tooMany: 'memory.import.problem.tooMany',
  invalidChars: 'memory.import.problem.invalidChars',
  unknownKind: 'memory.import.problem.unknownKind',
  unknownScope: 'memory.import.problem.unknownScope',
  needsProject: 'memory.import.problem.needsProject',
  wrongType: 'memory.import.problem.wrongType',
} as const satisfies Record<NotesImportProblem['code'], MessageKey>;

const FIELD_LABELS = {
  title: 'memory.editor.note.field.title',
  body: 'memory.editor.note.field.body',
  kind: 'memory.editor.note.field.kind',
  tags: 'memory.editor.note.field.tags',
  scope: 'memory.editor.scope',
} as const satisfies Record<NonNullable<NotesImportProblem['field']>, MessageKey>;

/** What is wrong with a row, for people: "Title: missing". A problem of no one field is said alone. */
export function problemText(problem: NotesImportProblem): string {
  const what = t(PROBLEM_LABELS[problem.code]);

  if (problem.field === null) {
    return what.charAt(0).toLocaleUpperCase(i18n.locale) + what.slice(1);
  }

  return t('memory.import.problem.field', { field: t(FIELD_LABELS[problem.field]), problem: what });
}

const REFUSAL_LABELS = {
  tooLarge: 'memory.import.refused.tooLarge',
  notUtf8: 'memory.import.refused.notUtf8',
  missingColumns: 'memory.import.refused.missingColumns',
  invalidJson: 'memory.import.refused.invalidJson',
  wrongJsonShape: 'memory.import.refused.wrongJsonShape',
  invalidCsv: 'memory.import.refused.invalidCsv',
  empty: 'memory.import.refused.empty',
} as const satisfies Record<NotesImportRefusal['reason'], MessageKey>;

/** Why a file was refused, for people; a CSV file that does not parse names the line it broke at. */
export function refusalText(refusal: NotesImportRefusal): string {
  if (refusal.reason === 'invalidCsv' && refusal.line !== null) {
    return t('memory.import.refused.invalidCsvLine', { line: refusal.line });
  }

  return t(REFUSAL_LABELS[refusal.reason]);
}

/** How many notes the import wrote, whichever way. */
export function writtenCount(result: NotesImportResult): number {
  return result.created + result.updated + result.replaced;
}
