import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeListFilter } from '../bindings/KnowledgeListFilter';
import { numberOf } from './knowledge-routing';

/** The documents a list shows: by repository scope, format, place, state and the words of the search (in the title). A live document counts for the state of its pending version too (`pendingOf` finds it). */
export function visibleDocuments(documents: KnowledgeDocument[], filter: KnowledgeListFilter | null, query: string | null, pendingOf: (doc: KnowledgeDocument) => KnowledgeDocument | undefined = () => undefined): KnowledgeDocument[] {
  const scope = filter?.scope ?? { kind: 'all' };
  const needle = (query ?? '').trim().toLowerCase();

  return documents.filter((doc) => {
    if (scope.kind === 'global' && doc.links.length > 0) {
      return false;
    }

    if (scope.kind === 'repo' && !doc.links.some((link) => link.repo === scope.key)) {
      return false;
    }

    if (filter?.format && doc.format !== filter.format) {
      return false;
    }

    if (filter?.place && doc.place !== filter.place) {
      return false;
    }

    // As the server keeps them: used at least once, or shown at least three times and never used (a document nobody rated has no counts).
    if (filter?.useful === 'ever' && (doc.useful ?? 0) < 1) {
      return false;
    }

    if (filter?.useful === 'never' && ((doc.shown ?? 0) < 3 || (doc.useful ?? 0) > 0)) {
      return false;
    }

    if (filter?.status && !inState(doc, pendingOf(doc), filter.status)) {
      return false;
    }

    return needle === '' || doc.title.toLowerCase().includes(needle) || doc.sourceName.toLowerCase().includes(needle);
  });
}

/** The sort of a list: newest first unless asked, the title, format and repository from A, the size, chunks, paths and usefulness from the largest (a document nobody rated counts 0, ties go by id the way the sort goes). */
export function sortDocuments(documents: KnowledgeDocument[], filter: KnowledgeListFilter | null): KnowledgeDocument[] {
  const sort = filter?.sort ?? 'updated';
  const natural = sort === 'title' || sort === 'format' || sort === 'repos' ? 'asc' : 'desc';
  const direction = (filter?.order ?? natural) === 'asc' ? 1 : -1;

  const key = (doc: KnowledgeDocument): string | number => {
    switch (sort) {
      case 'title':
        return doc.title.toLowerCase();
      case 'size':
        return doc.bytes;
      case 'updated':
        return Date.parse(doc.updatedAt);
      case 'format':
        return doc.format;
      case 'chunks':
        return doc.chunkCount;
      case 'paths':
        return doc.links.reduce((sum, link) => sum + link.globs.length, 0);
      case 'repos':
        return doc.links.map((link) => link.repo).sort()[0] ?? '';
      case 'useful':
        return doc.useful ?? 0;
      case 'lastUseful':
        return doc.lastUsefulAt === null || doc.lastUsefulAt === undefined ? 0 : Date.parse(doc.lastUsefulAt);
    }
  };

  return [...documents].sort((left, right) => {
    const a = key(left);
    const b = key(right);

    const tie = sort === 'useful' || sort === 'lastUseful' ? numberOf(left.id) - numberOf(right.id) : 0;

    return (a < b ? -1 : a > b ? 1 : tie) * direction;
  });
}

/** Whether a row is in the state a list asks for: its own, or for a live row its pending version's (`live` keeps live rows with or without one). */
function inState(doc: KnowledgeDocument, pending: KnowledgeDocument | undefined, status: NonNullable<KnowledgeListFilter['status']>): boolean {
  switch (status) {
    case 'live':
      return doc.status === 'live';
    case 'pending':
      return doc.status === 'live' && pending !== undefined;
    default:
      return doc.status === status || (doc.status === 'live' && pending?.status === status);
  }
}
