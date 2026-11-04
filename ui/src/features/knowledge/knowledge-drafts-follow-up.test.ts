import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
import { knowledgeReviewDraft } from '$lib/stores/knowledgeReviewDraft.svelte';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { diffMode } from '../changes/diff-mode.svelte';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  resetKnowledge();
  knowledgeReview.reset();
  knowledgeReviewDraft.reset();
  knowledgeViewer.reset();
  diffMode.set('unified');
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

describe('who the user is', () => {
  it('never offers an author to approve what they wrote, from the first look and before any refusal', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();

    const panel = await open('Support escalation guide');

    expect(await within(panel).findByText('Waiting for a teammate to review')).toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Approve' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Request changes' })).toBeNull();
    expect(calls(invoke, 'knowledge_doc_transition')).toHaveLength(0);
  });

  it('knows the user from the pulse too, so the rail has the name before the stats are read', async () => {
    fakeKnowledge('drafts');
    const stop = knowledgeChanges.start();

    await waitFor(() => expect(knowledgeViewer.name).toBe('Ada Lovelace'));
    expect(knowledgeViewer.is('ada lovelace')).toBe(true);
    expect(knowledgeViewer.is('Grace Hopper')).toBe(false);
    stop();
  });

  it('says up front that a user with no git name cannot approve, ask for changes or ask for approval', async () => {
    fakeKnowledge('drafts', false, {}, false, { name: null });
    await show();

    const theirs = await open('Data retention policy');
    const approve = await within(theirs).findByRole('button', { name: 'Approve' });

    expect(approve).toBeDisabled();
    expect(within(theirs).getByRole('button', { name: 'Request changes' })).toBeDisabled();
    expect(within(theirs).getByText(/This needs a git user\.name/)).toBeInTheDocument();

    const draft = await open('Rollback runbook');

    expect(await within(draft).findByRole('button', { name: 'Ask for approval' })).toBeDisabled();
  });
});

describe('taking an ask back', () => {
  it('lets the author of a document that waits take it back, and the history says so', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();

    const panel = await open('Support escalation guide');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Take it back' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_transition').at(-1)).toMatchObject({ action: 'takeBack' }));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Taken back'));
    expect(await within(panel).findByRole('button', { name: 'Ask for approval' })).toBeEnabled();
    expect(await within(panel).findByText(/took the request back/)).toBeInTheDocument();
  });

  it('offers it to nobody but the author', async () => {
    fakeKnowledge('drafts');
    await show();

    const theirs = await open('Data retention policy');

    expect(await within(theirs).findByRole('button', { name: 'Approve' })).toBeEnabled();
    expect(within(theirs).queryByRole('button', { name: 'Take it back' })).toBeNull();
  });
});

describe('the review queue', () => {
  it('sorts the Added by column on the server', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();
    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));

    await fireEvent.click(await screen.findByRole('button', { name: 'Added by' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_held_list').at(-1)).toMatchObject({ sort: 'addedBy', order: 'asc', offset: 0 }));
  });
});

describe('the saved comments of a review', () => {
  async function review(): Promise<HTMLElement> {
    fakeKnowledge('drafts');
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));

    return screen.findByRole('list', { name: /Changes in/ });
  }

  const THREAD = 'Why five retries when the contract says three?';

  it('sit under the line they are about, with the answer, and not in a panel of their own', async () => {
    const diff = await review();

    await within(diff).findByText(THREAD);

    expect(within(diff).getByText('The contract changed in September, the old number is gone.')).toBeInTheDocument();
    expect(within(diff).getByRole('button', { name: 'Reply' })).toBeEnabled();
    // The overall comment has no line, so it is the only one that stays in the block under the diff.
    expect(within(screen.getByLabelText('Comments on the changes')).queryByText(THREAD)).toBeNull();
    expect(within(screen.getByLabelText('Comments on the changes')).getByText(/The retry rules look right/)).toBeInTheDocument();
  });

  it('go under their own half in the split view', async () => {
    diffMode.set('split');
    const diff = await review();

    const thread = await within(diff).findByText(THREAD);

    expect(thread.closest('[data-half]')?.getAttribute('data-half')).toBe('new');
  });

  it('answer and resolve from where they sit', async () => {
    const diff = await review();

    await within(diff).findByText(THREAD);
    await fireEvent.click(within(diff).getAllByRole('button', { name: 'Resolve' })[0]);

    await waitFor(() => expect(screen.queryAllByText('Resolved').length).toBeGreaterThan(0));
  });

  it('shows what is outdated or has no line here under the diff, marked', async () => {
    const diff = await review();

    await within(diff).findByText(THREAD);
    await fireEvent.click(await screen.findByRole('option', { name: /Configuration/ }));

    const block = await screen.findByLabelText('Comments on the changes');

    expect(within(block).getByText('Keep the old default here.')).toBeInTheDocument();
    expect(within(block).getByText('Outdated')).toBeInTheDocument();
  });
});

describe('finishing a review of a document', () => {
  it('says so in the words of a document, and grows its overall comment like the other comment fields', async () => {
    fakeKnowledge('drafts');
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Finish review' }));

    expect(await screen.findByRole('heading', { name: 'Finish your review' })).toBeInTheDocument();

    const field = screen.getByRole('textbox', { name: /Overall comment/ });

    expect(field.getAttribute('rows')).toBe('4');
    expect(screen.getByText('Required unless you comment on lines')).toBeInTheDocument();
    expect(screen.getByRole('searchbox', { name: 'Search sections and comments' })).toBeInTheDocument();
  });
});
