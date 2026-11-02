import type { AuthorRef } from '../bindings/AuthorRef';

/** The people the sample documents, notes and scripts were written by, the way the servers print them: a user's own write has no `via`. */
export const ADA: AuthorRef = { name: 'Ada Lovelace', via: null };
export const GRACE: AuthorRef = { name: 'Grace Hopper', via: null };
export const GRACE_AGENT: AuthorRef = { name: 'Grace Hopper', via: 'claude-code' };

/** Who added and last changed a sample entry. Every fourth has no author (no git name was known), an even one was written by an agent. */
export function authorsOf(id: number): { addedBy: AuthorRef | null; updatedBy: AuthorRef | null } {
  if (id % 4 === 0) {
    return { addedBy: null, updatedBy: null };
  }

  const agent = id % 2 === 0;

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
