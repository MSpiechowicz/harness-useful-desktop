import { readJson, writeJson } from '$lib/utils/persisted';

/** One column of a `DataTable`. */
export interface Column {
  /** Names the column to the cell snippet and to `onsort`. */
  key: string;
  label: string;
  sortable?: boolean;
  /** A CSS width such as `8rem`; the column without one takes what the others leave. */
  width?: string;
  /** The smallest width of the table, as a container-query size, at which the column is shown; the first columns stay at any width. */
  from?: 'sm' | 'md' | 'lg';
  align?: 'end';
  /** A column of times or counts starts with the biggest, the others with the first in the alphabet. */
  descFirst?: boolean;
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

export function saveColumnWidths(tableId: string, widths: ColumnWidths): void {
  writeJson(widthsKey(tableId), widths);
}
