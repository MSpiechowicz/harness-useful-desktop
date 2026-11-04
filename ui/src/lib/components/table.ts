import { readJson, writeJson } from '$lib/utils/persisted';

/** One column of a `DataTable`. */
export interface Column {
  /** Names the column to the cell snippet and to `onsort`. */
  key: string;
  label: string;
  sortable?: boolean;
  /** A CSS width such as `8rem`; the column without one takes what the others leave, and is never narrower than `MIN_FLEX_PX`. */
  width?: string;
  align?: 'end';
  /** A column of times or counts starts with the biggest, the others with the first in the alphabet. */
  descFirst?: boolean;
}

/** The widths columns of the same kind share in every table (Notes, Scripts, Documents), so who added an entry and how useful it was read alike wherever they are. */
export const COLUMN_WIDTH = { author: '12rem', useful: '13rem', lastUseful: '9rem', project: '18rem' } as const;

/* `project` is the column that names the projects a row applies to (Projects, Scope): room for an `owner/repository` name. */

/** What the column without a width of its own, the one that takes what the others leave, is never squeezed below. */
export const MIN_FLEX_PX = 160;

/**
 * The width below which a table scrolls sideways instead of squeezing its columns: the sum of their widths, where a
 * column without a width counts as `MIN_FLEX_PX`. A column never hides at a small width, the table scrolls.
 */
export function minTableWidth(widths: readonly (string | undefined)[]): string {
  return `calc(${widths.map((width) => width ?? `${MIN_FLEX_PX}px`).join(' + ')})`;
}

export interface Sort {
  key: string;
  desc: boolean;
}

/** The sort after a click on a header: the same column flips its direction, another one starts in its natural direction. A table not sorted yet (null) starts in the natural direction too. */
export function nextSort(current: Sort | null, column: Pick<Column, 'key' | 'descFirst'>): Sort {
  return current?.key === column.key ? { key: column.key, desc: !current.desc } : { key: column.key, desc: column.descFirst === true };
}

/** The `aria-sort` of a sortable column's header. */
export function ariaSort(sort: Sort | null, key: string): 'ascending' | 'descending' | 'none' {
  if (sort?.key !== key) {
    return 'none';
  }

  return sort.desc ? 'descending' : 'ascending';
}

/** The narrowest a column can be dragged to. */
export const MIN_COLUMN_PX = 64;

/** The width in px that the user gave to each of the columns of one table, by column key. */
export type ColumnWidths = Record<string, number>;

const widthsKey = (tableId: string) => `harness:table:${tableId}`;

function isWidths(value: unknown): value is ColumnWidths {
  return typeof value === 'object' && value !== null && Object.values(value).every((width) => typeof width === 'number' && Number.isFinite(width));
}

/** The widths kept for a table, without those of columns it no longer has or that are narrower than allowed. */
export function readColumnWidths(tableId: string, columns: Column[]): ColumnWidths {
  const saved = readJson<ColumnWidths>(widthsKey(tableId), {}, isWidths);
  const kept = columns.filter((column) => saved[column.key] >= MIN_COLUMN_PX);

  return Object.fromEntries(kept.map((column) => [column.key, saved[column.key]]));
}

/**
 * Keeps the widths of the columns on show. The widths kept for columns that are not on show now (a column that appears
 * only when there is data for it) stay in storage; a column on show without a width in `widths` was reset, so its width goes.
 */
export function saveColumnWidths(tableId: string, widths: ColumnWidths, columns: Column[]): void {
  const stored = readJson<ColumnWidths>(widthsKey(tableId), {}, isWidths);
  const shown = new Set(columns.map((column) => column.key));
  const hidden = Object.entries(stored).filter(([key]) => !shown.has(key));
  const own = Object.entries(widths).filter(([key]) => shown.has(key));

  writeJson(widthsKey(tableId), Object.fromEntries([...hidden, ...own]));
}

/** The widths kept for the columns that `current` has no width for yet: a column that appeared after the table was shown. */
export function widthsForNewColumns(tableId: string, columns: Column[], current: ColumnWidths): ColumnWidths {
  const saved = readColumnWidths(tableId, columns);

  return Object.fromEntries(Object.entries(saved).filter(([key]) => current[key] === undefined));
}
