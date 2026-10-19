import { cx } from '$lib/utils/cx';

const FIELD = 'w-full rounded-md border bg-surface-2 text-base text-fg outline-none placeholder:text-fg-3 focus:border-focus';

/** The classes of a text field of a document's editor: `shape` is its size (`h-control px-3` for a line, `resize-none px-3 py-2` for a text area). */
export function fieldClass(invalid: boolean, shape: string = 'h-control px-3', extra?: string): string {
  return cx(FIELD, shape, extra, invalid ? 'border-danger' : 'border-line-strong');
}
