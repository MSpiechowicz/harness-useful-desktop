import type { FactRow } from '$lib/components/facts';
import { t } from '$lib/i18n/i18n.svelte';
import type { AuthorRef } from '$lib/ipc/bindings/AuthorRef';

/** The "via claude-code" after a name, for an agent's write: nothing for the user's own. */
export function viaText(author: AuthorRef): string | null {
  return author.via ? t('memory.author.via', { via: author.via }) : null;
}

/** Who wrote it in one line: "Ada Lovelace" or "Ada Lovelace via claude-code". */
export function authorLine(author: AuthorRef): string {
  const via = viaText(author);

  return via ? `${author.name} ${via}` : author.name;
}

function sameAuthor(a: AuthorRef, b: AuthorRef): boolean {
  return a.name === b.name && (a.via ?? null) === (b.via ?? null);
}

interface Authored {
  createdAt: string;
  updatedAt: string;
  addedBy?: AuthorRef | null;
  updatedBy?: AuthorRef | null;
}

/** "Added by" before the moments and "Updated by" after them. A name that is not known has no row, and the same author who never changed it again is not said twice. */
export function addedByFact(item: Pick<Authored, 'addedBy'>): FactRow[] {
  return item.addedBy ? [{ label: t('memory.detail.addedBy'), value: authorLine(item.addedBy) }] : [];
}

export function updatedByFact(item: Authored): FactRow[] {
  const { addedBy, updatedBy } = item;

  if (!updatedBy) {
    return [];
  }

  if (addedBy && sameAuthor(addedBy, updatedBy) && item.updatedAt === item.createdAt) {
    return [];
  }

  return [{ label: t('memory.detail.updatedBy'), value: authorLine(updatedBy) }];
}
