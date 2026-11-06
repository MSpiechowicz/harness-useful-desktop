import type { AuthorFacet } from '../bindings/AuthorFacet';
import type { AuthorFilter } from '../bindings/AuthorFilter';
import type { AuthorRef } from '../bindings/AuthorRef';

/** The people the sample documents, notes and scripts were written by, the way the servers print them: a user's own write has no `via`. */
export const ADA: AuthorRef = { name: 'Ada Lovelace', via: null };
export const GRACE: AuthorRef = { name: 'Grace Hopper', via: null };
export const GRACE_AGENT: AuthorRef = { name: 'Grace Hopper', via: 'claude-code' };

/** Who added and last changed a sample entry. Every fourth has no author (no git name was known), every seventh was added by a second person, an even one was written by an agent. */
export function authorsOf(id: number): { addedBy: AuthorRef | null; updatedBy: AuthorRef | null } {
  if (id % 4 === 0) {
    return { addedBy: null, updatedBy: null };
  }

  const agent = id % 2 === 0;

  if (id % 7 === 0) {
    return { addedBy: agent ? GRACE_AGENT : GRACE, updatedBy: ADA };
  }

  return { addedBy: agent ? { ...ADA, via: 'claude-code' } : ADA, updatedBy: agent ? { ...ADA, via: 'claude-code' } : GRACE };
}

/** `entry` as a server without `notes-authors` sends it: the authors are not there. */
export function withoutAuthors<T extends { addedBy?: AuthorRef | null; updatedBy?: AuthorRef | null }>(entry: T): T {
  const { addedBy: _added, updatedBy: _updated, ...rest } = entry;

  return rest as T;
}

/** Browser-preview switch `?authors=old`: a Harness Memory without `notes-authors`, which sends no author of a note or a script. */
export function notesAuthorsListed(search: string = globalThis.location?.search ?? ''): boolean {
  return new URLSearchParams(search).get('authors') !== 'old';
}

/** Browser-preview switch `?authorFilter=old`: a Harness Memory that tells who wrote notes and scripts but cannot filter or sort by it, and sends no facet. */
export function notesAuthorFilterListed(search: string = globalThis.location?.search ?? ''): boolean {
  return new URLSearchParams(search).get('authorFilter') !== 'old';
}

/** The entries `author` keeps, as the servers do: those first added under exactly that name, or those with no known name. Every entry for no filter. */
export function keepAuthor<T extends { addedBy?: AuthorRef | null }>(entries: T[], author: AuthorFilter | null | undefined): T[] {
  if (!author) {
    return entries;
  }

  return entries.filter((entry) => (author.kind === 'unknown' ? !entry.addedBy : entry.addedBy?.name === author.name));
}

/** The authors facet of `entries`, which are the ones under every filter but the author's: people with the most entries first, then by name, and how many have no name. */
export function authorFacetOf(entries: { addedBy?: AuthorRef | null }[]): AuthorFacet {
  const counts = new Map<string, number>();
  let unknown = 0;

  for (const { addedBy } of entries) {
    if (addedBy) {
      counts.set(addedBy.name, (counts.get(addedBy.name) ?? 0) + 1);
    } else {
      unknown += 1;
    }
  }

  const people = [...counts].map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count || (foldedName(a.name) < foldedName(b.name) ? -1 : foldedName(a.name) > foldedName(b.name) ? 1 : 0));

  return { people, unknown };
}

/** A name as `--sort addedBy` compares it: only ASCII letters in lower case, the rest by code point, so "Émile" sorts after "zed" as on the servers. */
function foldedName(name: string): string {
  return name.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

/**
 * How `--sort addedBy` orders two entries in `direction` (1 from A, -1 from Z): named ones by the folded name, ties by id in the same direction,
 * and the entries nobody is known to have added last in both directions.
 */
export function compareAddedBy<T extends { addedBy?: AuthorRef | null }>(a: T, b: T, direction: 1 | -1, idOf: (entry: T) => number): number {
  const byId = direction * (idOf(a) - idOf(b));

  if (!a.addedBy || !b.addedBy) {
    return !a.addedBy && !b.addedBy ? byId : a.addedBy ? -1 : 1;
  }

  const [left, right] = [foldedName(a.addedBy.name), foldedName(b.addedBy.name)];

  return left === right ? byId : direction * (left < right ? -1 : 1);
}
