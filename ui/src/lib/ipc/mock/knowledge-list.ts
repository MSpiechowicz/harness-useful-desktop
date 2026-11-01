import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeListFilter } from '../bindings/KnowledgeListFilter';

/** The documents a list shows: by repository scope, format and the words of the search (in the title). */
export function visibleDocuments(documents: KnowledgeDocument[], filter: KnowledgeListFilter | null, query: string | null): KnowledgeDocument[] {
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

    return needle === '' || doc.title.toLowerCase().includes(needle) || doc.sourceName.toLowerCase().includes(needle);
  });
}

/** The sort of a list: newest first unless asked, the title from A, the size from the largest. */
export function sortDocuments(documents: KnowledgeDocument[], filter: KnowledgeListFilter | null): KnowledgeDocument[] {
  const sort = filter?.sort ?? 'updated';
  const natural = sort === 'title' ? 'asc' : 'desc';
  const direction = (filter?.order ?? natural) === 'asc' ? 1 : -1;

  const key = (doc: KnowledgeDocument): string | number => {
    switch (sort) {
      case 'title':
        return doc.title.toLowerCase();
      case 'size':
        return doc.bytes;
      case 'updated':
        return Date.parse(doc.updatedAt);
    }
  };

  return [...documents].sort((left, right) => {
    const a = key(left);
    const b = key(right);

    return (a < b ? -1 : a > b ? 1 : 0) * direction;
  });
}
