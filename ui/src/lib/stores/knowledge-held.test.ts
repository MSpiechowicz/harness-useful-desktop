import { afterEach, describe, expect, it } from 'vitest';

import type { KnowledgeDecided } from '$lib/ipc/bindings/KnowledgeDecided';
import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
import { IpcFailure } from '$lib/ipc/transport';
import { sampleHeld } from '$lib/ipc/mock/knowledge-fixtures';

import { fakeTransport } from '../../../tests/helpers';
import { KnowledgeHeldStore } from './knowledgeHeld.svelte';
import { toasts } from './toasts.svelte';

afterEach(() => {
  toasts.items = [];
});

const ITEM: KnowledgeHeldItem = sampleHeld()[0];

/** A store whose server answers every decision with `answer` (or fails), and the decisions it was given. */
function deciding(answer: KnowledgeDecided | Error) {
  const decisions: unknown[] = [];
  const store = new KnowledgeHeldStore(
    fakeTransport({
      knowledge_held_decide: (args: unknown) => {
        decisions.push(args);

        return answer instanceof Error ? Promise.reject(IpcFailure.from({ code: 'process', message: answer.message })) : answer;
      },
      knowledge_held_list: () => ({ availability: { type: 'available' }, items: [], total: 0 }),
    }),
  );

  return { store, decisions };
}

const said = () => toasts.items.map((toast) => `${toast.tone}: ${toast.title}`);

describe('deciding about a proposal', () => {
  it('decides only the version that was shown, and says what the approval did', async () => {
    const { store, decisions } = deciding({ type: 'approved', docId: 11, status: 'added', title: ITEM.title, chunks: 4, embedding: 'done' });

    expect(await store.decide(ITEM, 'approve')).toBe('left');
    expect(decisions).toEqual([{ id: ITEM.id, decision: 'approve', revision: ITEM.revision }]);
    expect(said()).toEqual(['success: Added to Knowledge']);
  });

  it.each([
    ['updated', 'success: Document updated'],
    ['unchanged', 'success: Already up to date'],
  ] as const)('tells an approval that %s apart from one that added', async (status, toast) => {
    const { store } = deciding({ type: 'approved', docId: 2, status, title: ITEM.title, chunks: null, embedding: null });

    await store.decide(ITEM, 'approve');

    expect(said()).toEqual([toast]);
  });

  it('lets a rejected proposal go', async () => {
    const { store } = deciding({ type: 'rejected' });

    expect(await store.decide(ITEM, 'reject')).toBe('left');
    expect(said()).toEqual(['success: Proposal rejected']);
  });

  it('lets go of a proposal somebody decided already, and says so', async () => {
    const { store } = deciding({ type: 'notFound', message: 'decided already' });

    expect(await store.decide(ITEM, 'approve')).toBe('left');
    expect(said()).toEqual(['info: Already decided elsewhere']);
  });

  it('keeps a proposal that changed since it was shown, and asks to read the new version', async () => {
    const { store } = deciding({ type: 'changed', message: 'revision moved' });

    expect(await store.decide(ITEM, 'approve')).toBe('stay');
    expect(said()).toEqual(['warning: Proposal changed']);
  });

  it('keeps a proposal the server refused and says the reason it gave', async () => {
    const { store } = deciding({ type: 'refused', message: 'the file is too large' });

    expect(await store.decide(ITEM, 'approve')).toBe('stay');
    expect(toasts.items.map((toast) => [toast.title, toast.description])).toEqual([['Harness Knowledge refused the decision', 'the file is too large']]);
  });

  it('keeps a proposal when the call itself failed', async () => {
    const { store } = deciding(new Error('spawn failed'));

    expect(await store.decide(ITEM, 'reject')).toBe('stay');
    expect(said()).toEqual(['danger: Could not reject the proposal']);
  });

  it('does not decide the same proposal twice at once', async () => {
    const { store, decisions } = deciding({ type: 'rejected' });

    await Promise.all([store.decide(ITEM, 'reject'), store.decide(ITEM, 'reject')]);

    expect(decisions).toHaveLength(1);
    expect(store.busy.size).toBe(0);
  });
});
