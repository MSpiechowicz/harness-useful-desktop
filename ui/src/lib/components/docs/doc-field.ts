import { cx } from '$lib/utils/cx';

const FIELD = 'w-full rounded-md border bg-surface-2 text-base text-fg outline-none placeholder:text-fg-3 focus:border-focus';

/** The classes of a text field of a document's editor: `shape` is its size (`h-control px-3` for a line, `resize-none px-3 py-2` for a text area). */
export function fieldClass(invalid: boolean, shape: string = 'h-control px-3', extra?: string): string {
  return cx(FIELD, shape, extra, invalid ? 'border-danger' : 'border-line-strong');
}

/** The classes of the button that opens a menu of choices in a document's editor (see `DocSelect`): it looks like a text field and shows the choice made. */
export const SELECT_TRIGGER =
  'flex h-control w-full items-center gap-2.5 rounded-md border border-line-strong bg-surface-2 px-3 text-left text-base text-fg transition-colors duration-[var(--dur-fast)] hover:bg-surface-3 focus-visible:border-focus disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-surface-2';
