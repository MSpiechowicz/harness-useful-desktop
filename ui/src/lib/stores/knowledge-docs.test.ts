import { afterEach, describe, expect, it } from 'vitest';

import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgePage } from '$lib/ipc/bindings/KnowledgePage';
import { IpcFailure } from '$lib/ipc/transport';
import { documentOf } from '$lib/ipc/mock/knowledge-fixtures';

import { fakeTransport } from '../../../tests/helpers';
import { type DocsRequest, docsKey, KNOWLEDGE_PAGE_SIZE, KnowledgeDocsStore, scopeOf } from './knowledgeDocs.svelte';
import { repoReach } from './memory-reach';
import { toasts } from './toasts.svelte';

afterEach(() => {
  toasts.items = [];
});

const REQUEST: DocsRequest = { query: '', scope: 'all', format: null, sort: 'updated', order: 'desc', page: 0 };
const doc = (id: number): KnowledgeDocument => documentOf({ id, title: `Doc ${id}`, name: `doc-${id}.md`, format: 'markdown', kb: 1, chunks: 1, age: id });

function listing(documents: KnowledgeDocument[]) {
  const asked: unknown[] = [];

  const transport = fakeTransport({
    knowledge_docs_list: (args: { offset: number | null; limit: number | null }): KnowledgePage => {
      asked.push(args);

      return { availability: { type: 'available' }, items: documents.slice(args.offset ?? 0, (args.offset ?? 0) + (args.limit ?? 50)), total: documents.length };
    },
    knowledge_docs_delete: ({ ids }: { ids: (number | string)[] }) => {
      const before = documents.length;
      documents = documents.filter((candidate) => !ids.includes(candidate.id));

      return { deleted: before - documents.length };
    },
    knowledge_repos_list: () => ({ availability: { type: 'available' }, global: 0, repos: [] }),
  });

  return { transport, asked };
}

describe('the documents table asks the server for one page', () => {
  it('turns the page, the search, the filters and the sort into the call', async () => {
    const { transport, asked } = listing(Array.from({ length: 120 }, (_, index) => doc(index + 1)));
    const store = new KnowledgeDocsStore(transport);

    store.list.sync(docsKey({ ...REQUEST, query: '  runbook ', scope: repoReach('git:github.com/acme/widget'), format: 'pdf', sort: 'title', order: 'asc', page: 2 }), null);
    await store.list.refresh();

    expect(asked.at(-1)).toEqual({
      filter: { scope: { kind: 'repo', key: 'git:github.com/acme/widget' }, format: 'pdf', sort: 'title', order: 'asc' },
      offset: 2 * KNOWLEDGE_PAGE_SIZE,
      limit: KNOWLEDGE_PAGE_SIZE,
      query: 'runbook',
    });
    expect(store.list.data?.items).toHaveLength(20);
  });

  it('sends no query for a blank search, and reads the global documents or every document by the scope choice', async () => {
    const { transport, asked } = listing([doc(1)]);
    const store = new KnowledgeDocsStore(transport);

    store.list.sync(docsKey({ ...REQUEST, query: '   ', scope: 'global' }), null);
    await store.list.refresh();

    expect(asked.at(-1)).toMatchObject({ query: null, filter: { scope: { kind: 'global' } } });
    expect(scopeOf('all')).toEqual({ kind: 'all' });
    // A project Harness Knowledge has no repository key for names no repository to filter by.
    expect(scopeOf(repoReach('local:/work/legacy'))).toEqual({ kind: 'all' });
  });

  it('reads the new page when the request changes and drops the answer to an older one', async () => {
    const { transport } = listing(Array.from({ length: 60 }, (_, index) => doc(index + 1)));
    const store = new KnowledgeDocsStore(transport);

    store.list.sync(docsKey(REQUEST), null);
    store.list.sync(docsKey({ ...REQUEST, page: 1 }), null);
    await store.list.refresh();

    expect(store.list.data?.items.map((item) => item.id)).toEqual(Array.from({ length: 10 }, (_, index) => 51 + index));
  });
});

describe('what changes a document', () => {
  it('reads the list again after a delete, and says nothing when it worked', async () => {
    const { transport } = listing([doc(1), doc(2), doc(3)]);
    const store = new KnowledgeDocsStore(transport);

    store.list.sync(docsKey(REQUEST), null);
    await store.list.refresh();

    expect(await store.remove([2])).toEqual({ deleted: 1 });
    expect(store.list.data?.items.map((item) => item.id)).toEqual([1, 3]);
    expect(toasts.items).toHaveLength(0);
  });

  it('deletes everything in batches, whatever the table was narrowed to', async () => {
    const { transport, asked } = listing(Array.from({ length: 450 }, (_, index) => doc(index + 1)));
    const store = new KnowledgeDocsStore(transport);

    expect(await store.removeAll()).toEqual({ deleted: 450, total: 450 });
    expect(asked.filter((args) => (args as { filter: unknown }).filter === null)).toHaveLength(4);
  });

  it('stops with the reason when a page comes from a server that is not available, instead of taking it for an empty database', async () => {
    let deletes = 0;
    const store = new KnowledgeDocsStore(
      fakeTransport({
        knowledge_docs_list: () => ({ availability: { type: 'failed', message: 'the database is locked' }, items: [], total: 0 }),
        knowledge_docs_delete: () => (deletes += 1, { deleted: 0 }),
        knowledge_repos_list: () => ({ availability: { type: 'available' }, global: 0, repos: [] }),
      }),
    );

    await expect(store.removeAll()).rejects.toThrow('the database is locked');

    const outdated = new KnowledgeDocsStore(fakeTransport({ knowledge_docs_list: () => ({ availability: { type: 'outdated' }, items: [], total: 0 }), knowledge_repos_list: () => ({ availability: { type: 'available' }, global: 0, repos: [] }) }));

    await expect(outdated.removeAll()).rejects.toThrow('Harness Knowledge did not answer');
    expect(deletes).toBe(0);
  });

  it('answers fewer deleted than there were when the server stopped taking documents', async () => {
    const all = [doc(1), doc(2), doc(3)];
    const store = new KnowledgeDocsStore(
      fakeTransport({
        knowledge_docs_list: () => ({ availability: { type: 'available' }, items: all, total: 3 }),
        knowledge_docs_delete: () => ({ deleted: 0 }),
        knowledge_repos_list: () => ({ availability: { type: 'available' }, global: 0, repos: [] }),
      }),
    );

    expect(await store.removeAll()).toEqual({ deleted: 0, total: 3 });
  });

  it('says why in a toast when the server refused a rename, and answers null', async () => {
    const store = new KnowledgeDocsStore(fakeTransport({ knowledge_doc_rename: () => Promise.reject(IpcFailure.from({ code: 'invalidInput', message: 'a title has 1 to 300 characters' })) }));

    expect(await store.rename(1, '')).toBeNull();
    expect(toasts.items.map((toast) => [toast.title, toast.description, toast.tone])).toEqual([['Could not rename the document', 'a title has 1 to 300 characters', 'danger']]);
  });

  it('answers the links as the server stored them', async () => {
    const stored = [{ repo: 'git:github.com/acme/widget', globs: ['src/**'] }];
    const store = new KnowledgeDocsStore(
      fakeTransport({
        knowledge_doc_links_set: () => stored,
        knowledge_docs_list: () => ({ availability: { type: 'available' }, items: [], total: 0 }),
        knowledge_repos_list: () => ({ availability: { type: 'available' }, global: 0, repos: [] }),
      }),
    );

    expect(await store.setLinks(1, [{ repo: 'git:github.com/acme/widget', globs: [' src/** '] }])).toEqual(stored);
  });
});
