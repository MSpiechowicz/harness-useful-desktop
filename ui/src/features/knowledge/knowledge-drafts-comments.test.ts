import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
import { knowledgeReviewDraft } from '$lib/stores/knowledgeReviewDraft.svelte';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  resetKnowledge();
  knowledgeReview.reset();
  knowledgeReviewDraft.reset();
  knowledgeViewer.reset();
});

afterEach(() => {
  nav.go('chat');
  toasts.items = [];
  vi.restoreAllMocks();
});

const ADA = { name: 'Ada Lovelace' };

const QUESTION = 'Why five retries when the contract says three?';
const CHANGELOG = 'Mention the new limit in the changelog.';

/** Grace's own line comment on "Install", next to Ada's. */
function withGracesComment(store: ReturnType<typeof fakeKnowledge>['store']): void {
  const comment: KnowledgeComment = { id: 9, reviewId: 1, section: 'Install', occurrence: 1, side: 'new', from: 2, to: 2, body: CHANGELOG, by: 'Grace Hopper', at: new Date().toISOString(), parentId: null, revision: 6, outdated: false };

  store.review.comments.get('15')?.push(comment);
}

/** The preview's thread on "Install" (Ada's question, answered by Grace), with a second reply from Ada: the thread has two replies. */
function withAdasOwnReply(store: ReturnType<typeof fakeKnowledge>['store']): void {
  const reply: KnowledgeComment = { id: 8, reviewId: null, parentId: 2, body: 'I will look at the contract again.', by: ADA.name, at: new Date().toISOString(), revision: 6, outdated: false };

  store.review.comments.get('15')?.push(reply);
}

/** Opens the review of the pending update of Billing API and waits for the saved comments. */
async function review(): Promise<HTMLElement> {
  await show();
  await fireEvent.click(await screen.findByText('Billing API'));
  await fireEvent.click(await within(await screen.findByRole('region', { name: 'Billing API' })).findByRole('button', { name: 'Review changes' }));

  const diff = await screen.findByRole('list', { name: /Changes in/ });

  await within(diff).findByText(QUESTION);

  return diff;
}

/** The card of the thread that holds `text`. */
function cardOf(text: string): HTMLElement {
  return screen.getByText(text).closest('.shadow-raised') as HTMLElement;
}

describe('deleting your own review comment', () => {
  it('confirms with the number of replies that go with it, deletes it and reads the comments again', async () => {
    const { invoke, store } = fakeKnowledge('drafts', false, {}, false, ADA);
    withAdasOwnReply(store);
    await review();
    const reads = calls(invoke, 'knowledge_doc_comments').length;

    await fireEvent.click(within(cardOf(QUESTION)).getByRole('button', { name: 'Delete' }));

    const confirm = await screen.findByRole('dialog', { name: 'Delete this comment?' });

    expect(within(confirm).getByText('Its 2 replies are deleted with it. This cannot be undone.')).toBeInTheDocument();

    await fireEvent.click(within(confirm).getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_comment_delete').at(-1)).toMatchObject({ id: 15, commentId: 2 }));
    await waitFor(() => expect(screen.queryByText(QUESTION)).toBeNull());
    expect(screen.queryByText('The contract changed in September, the old number is gone.')).toBeNull();
    expect(screen.queryByText('I will look at the contract again.')).toBeNull();
    expect(calls(invoke, 'knowledge_doc_comments').length).toBeGreaterThan(reads);
    // The other threads stay.
    expect(screen.getByText('The retry rules look right. Two questions on the install steps.')).toBeInTheDocument();
  });

  it('deletes only the reply when a reply is deleted', async () => {
    const { invoke, store } = fakeKnowledge('drafts', false, {}, false, ADA);
    withAdasOwnReply(store);
    await review();

    const card = cardOf(QUESTION);

    // Grace's reply is not Ada's to delete.
    expect(within(card).getAllByRole('button', { name: /^Delete the reply of / })).toHaveLength(1);

    await fireEvent.click(within(card).getByRole('button', { name: /^Delete the reply of / }));

    const confirm = await screen.findByRole('dialog', { name: 'Delete this comment?' });

    expect(within(confirm).getByText('This cannot be undone.')).toBeInTheDocument();

    await fireEvent.click(within(confirm).getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_comment_delete').at(-1)).toMatchObject({ id: 15, commentId: 8 }));
    await waitFor(() => expect(screen.queryByText('I will look at the contract again.')).toBeNull());
    expect(screen.getByText(QUESTION)).toBeInTheDocument();
    expect(screen.getByText('The contract changed in September, the old number is gone.')).toBeInTheDocument();
  });

  it('offers nothing on what somebody else wrote', async () => {
    const { store } = fakeKnowledge('drafts', false, {}, false, ADA);
    withGracesComment(store);
    await review();

    expect(within(cardOf(CHANGELOG)).queryByRole('button', { name: 'Delete' })).toBeNull();
    expect(within(cardOf(CHANGELOG)).getByRole('button', { name: 'Reply' })).toBeEnabled();
  });

  it('is off for a role that can only read, like Reply, and says why', async () => {
    fakeKnowledge(
      'drafts',
      false,
      { knowledge_stats: () => ({ documents: 10, chunks: 0, embeddedChunks: 0, bytes: 0, globalDocuments: 0, repositories: 0, generation: 1, schema: 1, model: null, offline: false, remoteWritable: false, approval: true, me: ADA }) },
      false,
      ADA,
    );
    await review();

    expect(within(cardOf(QUESTION)).getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(within(cardOf(QUESTION)).getByText('Your role on Remote knowledge can only read')).toBeInTheDocument();
  });
});

describe('the lines a comment is on', () => {
  it('are quoted only where they are not on screen: not under their own line, but on an outdated comment, and never on an overall one', async () => {
    fakeKnowledge('drafts', false, {}, false, ADA);
    await review();

    // Under its own line the quote would repeat the line above the card.
    expect(within(cardOf(QUESTION)).queryByRole('region', { name: 'Commented lines' })).toBeNull();

    const overall = cardOf('The retry rules look right. Two questions on the install steps.');

    expect(within(overall).queryByRole('region', { name: 'Commented lines' })).toBeNull();

    await fireEvent.click(screen.getByRole('option', { name: /Configuration/ }));

    const quote = within(cardOf('Keep the old default here.')).getByRole('region', { name: 'Commented lines' });

    expect(within(quote).getByText('Retries default to 3.')).toBeInTheDocument();
    expect(within(quote).getByText('2')).toBeInTheDocument();
  });
});
