import { describe, expect, it } from 'vitest';

import { decisionPin } from '../../features/knowledge/review-model';
import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
import { idKey } from '$lib/ipc/knowledge-id';
import type { Transport } from '$lib/ipc/transport';

import { KnowledgeReviewStore } from './knowledgeReview.svelte';

const diff = (path: string) => ({ path, occurrence: 1, status: 'modified', hunks: [], additions: 1, deletions: 0 });
const settle = () => new Promise((resolve) => setTimeout(resolve, 5));
const comment = (id: number): KnowledgeComment => ({ id, body: `Comment ${id}`, at: '2026-11-01T10:00:00Z', revision: 1, outdated: false });

/** A server that answers document 1 at once and holds every answer for document 2 until the test lets it go. */
function twoDocuments() {
  const held: ((value: unknown) => void)[] = [];
  const answer = (id: number, since: number | null) => ({ type: 'done', value: { pending: id, revision: 2, truncated: false, sections: [diff(since === null ? `only-${id}` : `since-${id}`)] } });

  const transport = {
    invoke: (command: string, args: { id: number; since?: number | null }) => {
      const mine = args.id === 1;

      if (command === 'knowledge_doc_compare') {
        return mine ? Promise.resolve(answer(1, args.since ?? null)) : new Promise((resolve) => held.push(() => resolve(answer(2, args.since ?? null))));
      }

      if (command === 'knowledge_doc_comments') {
        return mine ? Promise.resolve([comment(1)]) : new Promise((resolve) => held.push(() => resolve([comment(2)])));
      }

      return new Promise(() => {});
    },
  } as unknown as Transport;

  return { store: new KnowledgeReviewStore(transport), release: () => held.splice(0).forEach((go) => go(null)) };
}

describe('what is read for one document is never shown for another', () => {
  it('gives no comparison of document 2 while it loads, then its own, and pins no verdict on document 1 meanwhile', async () => {
    const { store, release } = twoDocuments();

    store.compare.sync(idKey(1), null);
    await settle();

    expect(store.shownCompare(1)?.sections[0].path).toBe('only-1');
    expect(decisionPin(store.shownCompare(1), { id: 1, revision: 2 }).reason).toBeNull();

    store.compare.sync(idKey(2), null);

    // The loader keeps document 1's data until document 2's arrives: it is not document 2's.
    expect(store.compare.status).toBe('loading');
    expect(store.shownCompare(2)).toBeNull();
    expect(store.compareOutcome(2)).toBeNull();
    expect(decisionPin(store.shownCompare(2), { id: 2, revision: 2 })).toMatchObject({ revision: null, reason: 'The changes you would decide on are not on screen yet.' });

    release();
    await settle();

    expect(store.shownCompare(2)?.sections[0].path).toBe('only-2');
    expect(store.shownCompare(1)).toBeNull();
    expect(decisionPin(store.shownCompare(2), { id: 2, revision: 2 }).reason).toBeNull();
  });

  it('gives the comments and the comparison since a revision only for the document and revision they were read for', async () => {
    const { store, release } = twoDocuments();

    store.comments.sync(idKey(1), null);
    store.since.sync(`${idKey(1)}|1`, null);
    await settle();

    expect(store.commentsOf(1)?.map((each) => each.id)).toEqual([1]);
    expect(store.sinceOutcome(1, 1)).toMatchObject({ type: 'done' });
    expect(store.sinceOutcome(1, 2)).toBeNull();

    store.comments.sync(idKey(2), null);
    store.since.sync(`${idKey(2)}|1`, null);

    expect(store.commentsOf(2)).toBeNull();
    expect(store.commentsOf(1)).toBeNull();
    expect(store.sinceOutcome(2, 1)).toBeNull();
    expect(store.sinceOutcome(1, 1)).toBeNull();

    release();
    await settle();

    expect(store.commentsOf(2)?.map((each) => each.id)).toEqual([2]);
    expect(store.sinceOutcome(2, 1)).toMatchObject({ type: 'done' });
  });

  it('keeps showing what a quiet read of the same document is re-reading', async () => {
    const { store } = twoDocuments();

    store.compare.sync(idKey(1), null);
    await settle();
    const again = store.refresh();

    expect(store.shownCompare(1)).not.toBeNull();
    await again;
    expect(store.shownCompare(1)).not.toBeNull();
  });
});

describe('a comparison of another row cannot pin a verdict', () => {
  it('waits when the compare is of another row or of another revision, and goes when it is the version at its latest text', () => {
    const version = { id: 15, revision: 6 };

    expect(decisionPin({ pending: 12, revision: 6 }, version).reason).not.toBeNull();
    expect(decisionPin({ pending: 15, revision: 5 }, version).reason).not.toBeNull();
    expect(decisionPin({ pending: 'local:15', revision: 6 }, version).reason).not.toBeNull();
    expect(decisionPin({ pending: 15, revision: 6 }, version)).toEqual({ revision: 6, reason: null });
    // A server that prints neither is trusted as it always was.
    expect(decisionPin({}, version)).toEqual({ revision: 6, reason: null });
  });
});
