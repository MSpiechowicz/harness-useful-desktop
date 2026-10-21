import type { Sort } from '../table';

/** How many documents (hooks, routines) a page of their table holds. */
export const DOC_PAGE_SIZE = 50;

/** How many documents (hooks, routines) one scope, the global ones or one project's, can hold. */
export const MAX_PER_SCOPE = 64;

/** Whether a scope already holds the most documents it can: a document that stays in its scope (`stays`) is not counted twice. */
export function scopeIsFull(held: number, stays: boolean): boolean {
  return !stays && held >= MAX_PER_SCOPE;
}

/** What a column sorts a document by: a number, a text (in the reader's alphabet), or null for a document with nothing in that column. */
export type SortValue = string | number | null;

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

function compareValues(a: Exclude<SortValue, null>, b: Exclude<SortValue, null>): number {
  return typeof a === 'number' && typeof b === 'number' ? a - b : collator.compare(String(a), String(b));
}

/**
 * The documents in the order a column asks for. `null` leaves them as they are (the run order). A tie keeps that order too, and a
 * document with nothing in the column goes last, whichever way the sort runs.
 */
export function sortDocs<V>(views: V[], sort: Sort | null, values: Record<string, (view: V) => SortValue>): V[] {
  const valueOf = sort ? values[sort.key] : undefined;

  if (!sort || !valueOf) {
    return views;
  }

  const sign = sort.desc ? -1 : 1;

  return [...views].sort((a, b) => {
    const [first, second] = [valueOf(a), valueOf(b)];

    if (first === null || second === null) {
      return Number(first === null) - Number(second === null);
    }

    return compareValues(first, second) * sign;
  });
}

/** A column of a documents table: the classes of its `<col>` (width, when it shows) and of its header cell. */
export interface DocColumn {
  /** Names the column to `onsort`; a column with a key sorts. */
  key?: string;
  label: string;
  /** A column of switches or counts starts with the biggest, the others with the first in the alphabet. */
  descFirst?: boolean;
  col?: string;
  head?: string;
  /** The header's text lines up on the right, over a column of switches. */
  right?: boolean;
}

/** What a documents table lets its view do: put the focus on a row, e.g. back on the one whose panel was just closed. */
export interface DocTableHandle {
  focusRow: (id: string) => void;
}

/** What the buttons of a document's panel can ask for. */
export interface DocActionHandlers {
  onedit: () => void;
  ondelete: () => void;
  onshare: () => void;
  onupdate: () => void;
  onremove: () => void;
  oncopy: () => void;
}

/** What a list of documents lets its view do. */
export interface DocListHandlers<V> {
  onopen: (id: string) => void;
  /** Moves the open panel along when the arrow keys move the focus. */
  onmove: (id: string) => void;
  ontoggle: (view: V, enabled: boolean) => void;
  /** The table, for the view to put the focus back on a row once its panel is closed. */
  table?: DocTableHandle;
}
