import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { diffOf, numbered } from '$lib/ipc/mock/knowledge-review-diff';
import { knowledgeReplace } from '$lib/stores/knowledgeReplace.svelte';
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

async function open(title: string): Promise<HTMLElement> {
  await fireEvent.click(await screen.findByText(title));

  return screen.findByRole('region', { name: title });
}

/** The ask of the pending update of Billing API (row 15), as a teammate who is not its writer made it. */
function askedBy(store: ReturnType<typeof fakeKnowledge>['store'], asker: string): void {
  store.review.events.set('15', [
    { kind: 'asked', by: asker, at: new Date().toISOString(), comment: 'Please look at the retries.' },
    { kind: 'edited', by: 'Grace Hopper', at: new Date(Date.now() - 3_600_000).toISOString(), comment: null },
  ]);
}

describe('who owns a version that waits', () => {
  it('treats the one who asked for the review as its author too: Take it back, and no Approve', async () => {
    const { store } = fakeKnowledge('drafts', false, {}, false, { name: 'Ada Lovelace' });
    askedBy(store, 'Ada Lovelace');
    await show();

    const panel = await open('Billing API');

    expect(await within(panel).findByRole('button', { name: 'Take it back' })).toBeEnabled();
    expect(within(panel).queryByRole('button', { name: 'Approve' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Request changes' })).toBeNull();
  });

  it('still offers a third teammate to approve, and names the one who asked', async () => {
    const { store } = fakeKnowledge('drafts', false, {}, false, { name: 'Carol Shaw' });
    askedBy(store, 'Ada Lovelace');
    await show();

    const panel = await open('Billing API');

    expect(await within(panel).findByText('Ada Lovelace asked for your review')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Approve' })).toBeEnabled();
    expect(within(panel).queryByRole('button', { name: 'Take it back' })).toBeNull();
  });

  it('does not take the writer for the user when a refusal may have been about the asker', async () => {
    const refusal = { type: 'refused', code: 'selfApproval', message: 'a teammate has to decide' };
    const { store } = fakeKnowledge('drafts', false, { knowledge_doc_transition: () => refusal }, false, 'unknown');
    askedBy(store, 'Ada Lovelace');
    await show();

    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Approve' }));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('A teammate has to decide'));
    await waitFor(() => expect(knowledgeReview.busy.size).toBe(0));

    // Grace wrote it, Ada asked: the refusal does not say which of them the user is.
    expect(knowledgeViewer.name).toBeNull();
  });
});

describe('replacing a file that has the same text and asking at once', () => {
  const file = { token: 't', name: 'billing.yaml', bytes: 10 };
  const doc = { id: 2, links: [], title: 'Billing API' };
  const event = (asked: boolean) => ({ type: 'file', path: 'billing.yaml', status: 'unchanged', reason: null, docId: 2, chunks: 3, embedding: 'done', warnings: [], asked });
  const importing = (asked: boolean) => ({
    knowledge_import: (args: unknown) => {
      (args as { onEvent: (event: unknown) => void }).onEvent(event(asked));

      return { counts: { added: 0, updated: 0, unchanged: 1, skipped: 0, failed: 0 }, cancelled: false };
    },
  });

  it('says it asked, and counts as done', async () => {
    fakeKnowledge('drafts', false, importing(true));

    expect(await knowledgeReplace.replace(doc, file, { note: '' })).toBe(true);
    expect(toasts.items.map((toast) => toast.title)).toEqual(['Replaced and sent for approval']);
  });

  it('still says nothing changed when nothing was asked either', async () => {
    fakeKnowledge('drafts', false, importing(false));

    expect(await knowledgeReplace.replace(doc, file, { note: '' })).toBe(false);
    expect(toasts.items.map((toast) => toast.title)).toEqual(['Nothing changed']);
  });
});

describe('line comments are tied to the revision they were written on', () => {
  async function review(): Promise<void> {
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    await screen.findByRole('button', { name: /Finish review/ });
  }

  it('keeps them visible with a grey note, sends the drafted revision so the server says it changed, and keeps them on request', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();

    // Written when the update was at revision 5: it is at 6 now.
    knowledgeReviewDraft.add(2, 5, { section: 'Install', occurrence: 1 }, { side: 'new', startLine: 1, endLine: 1 }, 'Why five retries?');
    await review();

    expect(await screen.findByText('The document changed since you wrote these comments')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Finish review (1)' })).toBeEnabled();

    await fireEvent.click(screen.getByRole('button', { name: 'Finish review (1)' }));
    await fireEvent.input(await screen.findByRole('textbox', { name: /Overall/ }), { target: { value: 'Looks fine' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Comment' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_review').at(-1)).toMatchObject({ id: 15, revision: 5 }));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('It changed meanwhile'));

    const action = toasts.items.find((toast) => toast.title === 'It changed meanwhile')?.action;

    expect(action?.label).toBe('Keep my comments on the new version');
    expect(knowledgeReviewDraft.comments(2)).toHaveLength(1);

    action?.run();

    await waitFor(() => expect(screen.queryByText('The document changed since you wrote these comments')).toBeNull());
    expect(knowledgeReviewDraft.revision(2)).toBe(6);
  });

  it('shows no note for comments written on the current revision', async () => {
    fakeKnowledge('drafts');
    await show();

    knowledgeReviewDraft.add(2, 6, { section: 'Install', occurrence: 1 }, { side: 'new', startLine: 1, endLine: 1 }, 'Fine.');
    await review();

    expect(screen.queryByText('The document changed since you wrote these comments')).toBeNull();
  });
});

describe('Finish review while the document is read', () => {
  it('is off and says why', async () => {
    fakeKnowledge('drafts', false, { knowledge_doc_get: () => new Promise(() => {}) });
    await show();
    nav.go('knowledge');
    const { viewState } = await import('$lib/stores/view-state.svelte');

    viewState.knowledgeReviewId = 2;

    const finish = await screen.findByRole('button', { name: 'Finish review' });

    expect(finish).toBeDisabled();
    await fireEvent.pointerEnter(finish.parentElement?.parentElement as HTMLElement);
    expect(await screen.findByText('Loading the document')).toBeInTheDocument();
  });
});

describe('a read-only role in the saved comments', () => {
  it('has Reply and Resolve off with the reason in a grey line', async () => {
    fakeKnowledge('drafts', false, {
      knowledge_stats: () => ({
        documents: 10,
        chunks: 0,
        embeddedChunks: 0,
        bytes: 0,
        globalDocuments: 0,
        repositories: 0,
        generation: 1,
        schema: 1,
        model: null,
        offline: false,
        remoteWritable: false,
        approval: true,
      }),
    });
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    const diff = await screen.findByRole('list', { name: /Changes in/ });

    await within(diff).findByText('Why five retries when the contract says three?');

    expect(within(diff).getByRole('button', { name: 'Reply' })).toBeDisabled();
    expect(within(diff).getByRole('button', { name: 'Resolve' })).toBeDisabled();
    expect(within(diff).getAllByText('Your role on Remote knowledge can only read').length).toBeGreaterThan(0);
  });
});

describe('the history of a live document with a pending update', () => {
  it('joins the events of the live row and the pending row, and reads the review from the pending one', async () => {
    const { invoke, store } = fakeKnowledge('drafts');
    store.review.events.set('2', [{ kind: 'drafted', by: 'Zed Zephyr', at: new Date(Date.now() - 900 * 3_600_000).toISOString(), comment: null }]);
    await show();

    const panel = await open('Billing API');

    // The update's rounds and the live document's own life, in one timeline.
    expect(await within(panel).findByText('Zed Zephyr')).toBeInTheDocument();
    expect(within(panel).getAllByText(/asked for approval/).length).toBeGreaterThan(0);
    expect(
      calls(invoke, 'knowledge_doc_history')
        .map((args) => (args as { id: number }).id)
        .sort(),
    ).toEqual(expect.arrayContaining([2, 15]));
  });
});

describe('sections that share a heading', () => {
  const twice = {
    knowledge_doc_compare: () => ({
      type: 'done',
      value: { live: 2, pending: 15, revision: 6, truncated: false, sections: numbered([diffOf('Install', { added: ['First text.'] }), diffOf('Install', { added: ['Second text.'] })]) },
    }),
  };

  it('keep their comments apart', async () => {
    fakeKnowledge('drafts', false, { ...twice, knowledge_doc_comments: () => [] });
    await show();

    knowledgeReviewDraft.add(2, 6, { section: 'Install', occurrence: 2 }, { side: 'new', startLine: 1, endLine: 1 }, 'On the second one.');
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    const rows = await screen.findAllByRole('option');

    expect(rows).toHaveLength(2);

    // Only the second Install has the comment: the count and the comment follow it.
    expect(within(rows[0]).queryByText('1')).toBeNull();
    expect(within(rows[1]).getByText('1')).toBeInTheDocument();
    expect(screen.queryByText('On the second one.')).toBeNull();

    await fireEvent.click(rows[1]);

    expect(await screen.findByText('On the second one.')).toBeInTheDocument();
  });
});

describe('a saved comment on a section these changes do not list', () => {
  it('is shown with its section named, under the section on show', async () => {
    const onIntro = { id: 41, reviewId: 7, section: 'Intro', occurrence: 1, side: 'new', from: 3, to: 3, body: 'Name the rotation tool here.', by: 'Grace Hopper', at: '2026-11-01T10:00:00Z', parentId: null, resolvedAt: null, resolvedBy: null, revision: 6, outdated: false };
    fakeKnowledge('drafts', false, {
      knowledge_doc_compare: () => ({ type: 'done', value: { live: 2, pending: 15, revision: 6, truncated: false, sections: numbered([diffOf('Install', { added: ['First text.'] })]) } }),
      knowledge_doc_comments: () => [onIntro],
    });
    await show();

    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));

    expect(await screen.findByText('Name the rotation tool here.')).toBeInTheDocument();
    expect(screen.getByText('Comment on lines not shown here')).toBeInTheDocument();
    expect(screen.getByText('Intro · Line 3 (new)')).toBeInTheDocument();
  });
});

describe('the follow-up of a proposal', () => {
  it('asks whether the agent can be reached in the folder the proposal was about', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();
    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await fireEvent.click(await screen.findByText('Local database reset'));
    await screen.findByRole('region', { name: 'Local database reset' });

    await waitFor(() => expect(calls(invoke, 'knowledge_held_follow_up_available').length).toBeGreaterThan(0));
    expect(calls(invoke, 'knowledge_held_follow_up_available').at(-1)).toHaveProperty('projectPath');
  });
});
