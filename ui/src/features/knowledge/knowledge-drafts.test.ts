import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { documentOf } from '$lib/ipc/mock/knowledge-fixtures';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
import { knowledgeReviewDraft } from '$lib/stores/knowledgeReviewDraft.svelte';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  resetKnowledge();
  knowledgeViewer.reset();
  knowledgeReview.reset();
  knowledgeReviewDraft.reset();
});

afterEach(() => {
  nav.go('chat');
  toasts.items = [];
  vi.restoreAllMocks();
});

/** Opens the panel of a document of the table by its title and answers the panel. */
async function open(title: string): Promise<HTMLElement> {
  await fireEvent.click(await screen.findByText(title));

  return screen.findByRole('region', { name: title });
}

describe('the Status column and filter', () => {
  it('badges every row, live ones too, and asks the server for the chosen state from the first page', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();

    const row = (id: number) => within(document.querySelector<HTMLElement>(`[data-row="${id}"]`) as HTMLElement);

    expect(row(12).getByText('Draft')).toBeInTheDocument();
    expect(row(1).getByText('Live')).toBeInTheDocument();
    expect(row(2).getByText('Update waiting')).toBeInTheDocument();

    viewState.knowledgePage = 1;
    await fireEvent.click(screen.getByRole('button', { name: 'Status: All' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'Waiting for approval' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { status: 'waiting' }, offset: 0 }));
    expect(viewState.knowledgePage).toBe(0);
  });

  it('offers the Update waiting state in the Status filter and sends it to the server', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();

    await fireEvent.click(screen.getByRole('button', { name: 'Status: All' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'Update waiting' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { status: 'pending' }, offset: 0 }));
  });

  it('tells a teammate who may approve a row from one of its authors who waits', async () => {
    fakeKnowledge('drafts');
    await show();

    const row = (id: number) => within(document.querySelector<HTMLElement>(`[data-row="${id}"]`) as HTMLElement);

    // Data retention policy was written by Grace, Support escalation guide by Ada (the viewer) and Grace.
    expect(await row(11).findByText('Ready to review')).toBeInTheDocument();
    expect(row(11).queryByText('Waiting for approval')).toBeNull();
    expect(row(13).getByText('Waiting for approval')).toBeInTheDocument();
    expect(row(13).queryByText('Ready to review')).toBeNull();
  });

  it('offers neither for a Harness Knowledge that keeps no drafts', async () => {
    const { invoke } = fakeKnowledge('older');
    await show();

    expect(screen.queryByRole('button', { name: /^Status/ })).toBeNull();
    expect(screen.queryByRole('columnheader', { name: 'Status' })).toBeNull();
    expect(calls(invoke, 'knowledge_docs_list').at(-1)).not.toHaveProperty('filter.status');
  });
});

describe('a status filter that is applied', () => {
  afterEach(() => {
    viewState.knowledgeStatus = null;
  });

  it('keeps the Status filter and column on screen while the server has not said it keeps drafts and no row shows a state, until it is cleared', async () => {
    // The features are not known yet, and the page the filter answers has only live rows: nothing else says that drafts exist.
    fakeKnowledge('drafts', false, {
      knowledge_status: () => new Promise(() => {}),
      knowledge_docs_list: () => ({ availability: { type: 'available' }, items: [documentOf({ id: 1, title: 'Billing API', name: 'billing.md', format: 'markdown', kb: 1, chunks: 1, age: 1 })], total: 1, offline: false }),
    });
    viewState.knowledgeStatus = 'waiting';
    await show();

    expect(await screen.findByRole('button', { name: 'Status: Waiting for approval' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'Status' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Status filter' }));

    await waitFor(() => expect(screen.queryByRole('button', { name: /^Status/ })).toBeNull());
    expect(screen.queryByRole('columnheader', { name: 'Status' })).toBeNull();
  });
});

describe('the Review section of a document', () => {
  it('lets a teammate approve or ask for changes on what waits for them, and the author only wait', async () => {
    fakeKnowledge('drafts');
    await show();

    const theirs = await open('Data retention policy');

    expect(await within(theirs).findByText('Grace Hopper asked for your review')).toBeInTheDocument();
    expect(within(theirs).getByRole('button', { name: 'Approve' })).toBeEnabled();
    expect(within(theirs).getByRole('button', { name: 'Request changes' })).toBeEnabled();

    const mine = await open('Support escalation guide');

    expect(await within(mine).findByText('Waiting for a teammate to review')).toBeInTheDocument();
    expect(within(mine).queryByRole('button', { name: 'Approve' })).toBeNull();
    expect(within(mine).queryByRole('button', { name: 'Request changes' })).toBeNull();
  });

  it('offers the author of a version with changes requested to ask again or replace the file, in one row, and Replace file… only once in the panel', async () => {
    fakeKnowledge('drafts');
    await show();

    const panel = await open('Onboarding checklist');

    expect((await within(panel).findAllByText(/The accounts step is out of date/)).length).toBeGreaterThan(0);
    expect(within(panel).getByRole('button', { name: 'Ask for approval again' })).toBeEnabled();
    expect(within(panel).getAllByRole('button', { name: 'Replace file…' })).toHaveLength(1);
    expect(within(panel).queryByRole('button', { name: 'Approve' })).toBeNull();
    expect(within(panel).getAllByRole('textbox')[0].getAttribute('rows')).toBe('4');
  });

  it('offers to go live, not to ask, while team approval is off', async () => {
    fakeKnowledge('solo');
    await show();

    const panel = await open('Rollback runbook');

    expect(await within(panel).findByRole('button', { name: 'Go live' })).toBeEnabled();
    expect(within(panel).queryByRole('button', { name: 'Ask for approval' })).toBeNull();
  });

  it('asks for a comment before it asks for changes, and for a server that does not say who the user is, learns it from the refusal of an author to approve', async () => {
    const { invoke } = fakeKnowledge('drafts', false, {}, false, 'unknown');
    await show();

    // The name of the user is not known yet: the version that waits is offered to approve, and the server refuses it because the user wrote it.
    const panel = await open('Support escalation guide');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Approve' }));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('A teammate has to decide'));
    expect(calls(invoke, 'knowledge_doc_transition').at(-1)).toMatchObject({ action: 'approve' });

    // The refusal taught the app who the user is, so the author's panel has no Approve any more.
    await waitFor(() => expect(within(panel).queryByRole('button', { name: 'Approve' })).toBeNull());

    const theirs = await open('Data retention policy');

    await fireEvent.click(await within(theirs).findByRole('button', { name: 'Request changes' }));
    expect(within(theirs).getAllByRole('button', { name: 'Request changes' }).at(-1)).toBeDisabled();

    await fireEvent.input(within(theirs).getByRole('textbox'), { target: { value: 'Name the retention period' } });
    await fireEvent.click(within(theirs).getAllByRole('button', { name: 'Request changes' }).at(-1) as HTMLElement);

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_transition').at(-1)).toMatchObject({ action: 'requestChanges', comment: 'Name the retention period' }));
  });

  it('turns every refusal of the server into a toast', async () => {
    fakeKnowledge('drafts');
    await show();
    const codes = ['notFound', 'changed', 'wrongState', 'approvalOn', 'approvalOff', 'selfApproval', 'notAuthor', 'noName', 'readOnly', 'commentRequired', 'sinceUnavailable', 'refused'] as const;

    const { sayRefusal } = await import('$lib/stores/knowledgeReview.svelte');

    for (const code of codes) {
      toasts.items = [];
      sayRefusal(code, 'the server said so');

      expect(toasts.items, code).toHaveLength(1);
      expect(toasts.items[0].title.length, code).toBeGreaterThan(0);
      expect(toasts.items[0].description.length, code).toBeGreaterThan(0);
    }
  });
});

describe('importing with drafts', () => {
  async function stage(): Promise<void> {
    await fireEvent.click(screen.getAllByRole('button', { name: 'Add documents' })[0]);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose files' }));
    await screen.findByRole('button', { name: 'Start import' });
  }

  it('saves as a draft and cannot change that while team approval is on, with the way to Settings', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();
    const stop = knowledgeChanges.start();
    await waitFor(() => expect(knowledgeChanges.pulse?.approval).toBe(true));

    await stage();

    const draft = await screen.findByRole('switch', { name: 'Save as draft' });

    expect(draft).toBeChecked();
    expect(draft).toBeDisabled();
    expect(screen.getByText(/Team approval is on, so every document starts as a draft/)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Change in Settings' }));
    expect(nav.current).toBe('settings');

    stop();
    expect(calls(invoke, 'knowledge_import')).toHaveLength(0);
  });

  it('lets the user choose to save as a draft while team approval is off, and sends it', async () => {
    const { invoke } = fakeKnowledge('solo');
    await show();
    const stop = knowledgeChanges.start();
    await waitFor(() => expect(knowledgeChanges.pulse?.approval).toBe(false));

    await stage();

    const draft = await screen.findByRole('switch', { name: 'Save as draft' });

    expect(draft).not.toBeChecked();
    expect(draft).toBeEnabled();
    expect(screen.getByText("Agents don't use drafts until they go live.")).toBeInTheDocument();

    await fireEvent.click(draft);
    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_import').at(-1)).toMatchObject({ options: { draft: true } }));
    stop();
  });
});

describe('the review of the changes', () => {
  it('sends the verdict with the overall comment and the comments on lines as they were written', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();

    // The pending update of Billing API: its panel offers the full review.
    const panel = await open('Billing API');

    knowledgeReviewDraft.add(2, 6, { section: 'Install', occurrence: 1 }, { side: 'new', startLine: 2, endLine: 3 }, 'Why five retries?');
    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));

    expect(await screen.findByRole('button', { name: 'Finish review (1)' })).toBeInTheDocument();
    expect(screen.getAllByText('Install').length).toBeGreaterThan(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Finish review (1)' }));
    await fireEvent.click(await screen.findByRole('radio', { name: /Request changes/ }));
    await fireEvent.input(screen.getByRole('textbox', { name: /Overall/ }), { target: { value: 'Please fix the retries' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Request changes' }));

    await waitFor(() =>
      expect(calls(invoke, 'knowledge_doc_review').at(-1)).toMatchObject({
        id: 15,
        review: { verdict: 'requestChanges', body: 'Please fix the retries', comments: [{ section: 'Install', side: 'new', from: 2, to: 3, body: 'Why five retries?' }] },
      }),
    );
  });

  it('lets the author only comment', async () => {
    fakeKnowledge('drafts', false, {}, false, { name: 'Grace Hopper' });
    await show();

    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Finish review' }));

    expect(await screen.findByRole('button', { name: 'Comment' })).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: /Approve/ })).toBeNull();
  });

  it('asks for a comment for the author on a line, not for the agent', async () => {
    fakeKnowledge('drafts');
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    await fireEvent.click((await screen.findAllByRole('button', { name: /^Comment on (old|new) line/ }))[0]);

    expect(await screen.findByPlaceholderText('Leave a comment for the author…')).toBeInTheDocument();
    expect(screen.queryByPlaceholderText('Leave a comment for the agent…')).toBeNull();
  });

  it('goes back to the documents on Escape', async () => {
    fakeKnowledge('drafts');
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    await screen.findByRole('button', { name: 'Finish review' });
    await fireEvent.keyDown(window, { key: 'Escape' });

    await waitFor(() => expect(viewState.knowledgeReviewId).toBeNull());
  });
});

describe('the review queue of agent proposals', () => {
  it('says who a proposal is for and through which harness, and what was asked of an agent', async () => {
    fakeKnowledge('drafts');
    await show();

    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));

    expect(await screen.findByRole('columnheader', { name: 'Added by' })).toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: 'Proposed by' })).toBeNull();
    expect(screen.getAllByText('Ada Lovelace').length).toBeGreaterThan(0);
    expect(screen.getAllByText('via claude-code').length).toBeGreaterThan(0);

    await fireEvent.click(screen.getByText('Postgres failover notes'));
    const panel = await screen.findByRole('region', { name: 'Postgres failover notes' });

    expect(within(panel).getByText('Say which replica is promoted first.')).toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Request changes' })).toBeNull();
  });

  it('asks the agent for changes only while its chat is open here, with the comment', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();
    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await fireEvent.click(await screen.findByText('Local database reset'));
    const panel = await screen.findByRole('region', { name: 'Local database reset' });

    await waitFor(() => expect(within(panel).getByRole('button', { name: 'Request changes' })).toBeEnabled());
    await fireEvent.click(within(panel).getByRole('button', { name: 'Request changes' }));
    await fireEvent.input(within(panel).getByRole('textbox'), { target: { value: 'Name the tables' } });
    await fireEvent.click(within(panel).getAllByRole('button', { name: 'Request changes' }).at(-1) as HTMLElement);

    await waitFor(() => expect(calls(invoke, 'knowledge_held_request_changes').at(-1)).toMatchObject({ comment: 'Name the tables' }));
  });
});
