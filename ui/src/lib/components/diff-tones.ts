import type { DiffLineKind } from '$lib/ipc/bindings/DiffLineKind';

/**
 * The one diff palette, shared by the Changes view and diff blocks in chat: tinted rows with green added and red
 * removed text (the `--shiki-token-inserted` / `--shiki-token-deleted` colors).
 */
export const DIFF_ROW: Record<DiffLineKind, string> = { add: 'bg-success-wash', remove: 'bg-danger-wash', context: '', noNewline: '' };

export const DIFF_TEXT: Record<DiffLineKind, string> = {
  add: 'text-[color:var(--shiki-token-inserted)]',
  remove: 'text-[color:var(--shiki-token-deleted)]',
  context: 'text-fg',
  noNewline: 'text-fg-3',
};
