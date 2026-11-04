import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { diffOf, numbered } from '$lib/ipc/mock/knowledge-review-diff';
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

const CHANGED = /The document changed, showing the latest version/;

/** A compare of the pending update of Billing API (row 15, at revision 6) that says it is of `shown.revision`: what a read that is a revision behind (or ahead) of the document detail answers. */
function compareAt(shown: { revision: number }): Record<string, (args: unknown) => unknown> {
  return {
    knowledge_doc_compare: () => ({ type: 'done', value: { live: 2, pending: 15, revision: shown.revision, truncated: false, sections: numbered([diffOf('Install', { added: ['Set the retry limit to 5.'] })]) } }),
  };
}

describe('a verdict is about the diff on screen', () => {
  it('waits in the panel while the diff is of another revision than the document, says why, and approves the revision it was shown', async () => {
    const shown = { revision: 5 };
    const { invoke } = fakeKnowledge('drafts', false, compareAt(shown));
    await show();
    const panel = await open('Billing API');

    const approve = await within(panel).findByRole('button', { name: 'Approve' });

    await waitFor(() => expect(within(panel).getByText(CHANGED)).toBeInTheDocument());
    expect(approve).toBeDisabled();
    expect(within(panel).getByRole('button', { name: 'Request changes' })).toBeDisabled();

    // The next read brings the diff of the latest version: the verdict is on again.
    shown.revision = 6;
    await knowledgeReview.refresh();

    await waitFor(() => expect(within(panel).getByRole('button', { name: 'Approve' })).toBeEnabled());
    expect(within(panel).queryByText(CHANGED)).toBeNull();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Approve' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_transition').at(-1)).toMatchObject({ id: 15, action: 'approve', revision: 6 }));
  });

  it('is never sent while the diff is not there, and says why in grey', async () => {
    const { invoke } = fakeKnowledge('drafts', false, { knowledge_doc_compare: () => new Promise(() => {}) });
    await show();
    const panel = await open('Billing API');

    const approve = await within(panel).findByRole('button', { name: 'Approve' });

    expect(approve).toBeDisabled();
    expect(within(panel).getByText('The changes you would decide on are not on screen yet.')).toBeInTheDocument();
    await fireEvent.click(approve);
    expect(calls(invoke, 'knowledge_doc_transition')).toHaveLength(0);
  });

  it('keeps Approve and Request changes of the review view off while the diff is of another revision, and lets a comment go', async () => {
    const shown = { revision: 5 };
    const { invoke } = fakeKnowledge('drafts', false, compareAt(shown));
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Finish review' }));
    await fireEvent.click(await screen.findByRole('radio', { name: /Approve/ }));

    const submit = screen.getAllByRole('button', { name: 'Approve' }).find((button) => button.closest('footer') !== null) as HTMLElement;

    expect(submit).toBeDisabled();
    expect(screen.getAllByText(CHANGED).length).toBeGreaterThan(0);

    await fireEvent.click(await screen.findByRole('radio', { name: /Request changes/ }));
    await fireEvent.input(screen.getByRole('textbox', { name: /Overall/ }), { target: { value: 'Please fix' } });
    expect(screen.getByRole('button', { name: 'Request changes' })).toBeDisabled();

    await fireEvent.click(screen.getByRole('radio', { name: /Comment/ }));
    expect(screen.getByRole('button', { name: 'Comment' })).toBeEnabled();
    expect(calls(invoke, 'knowledge_doc_review')).toHaveLength(0);

    // The diff of the latest version arrives: the review is sent on the revision it shows.
    shown.revision = 6;
    await knowledgeReview.refresh();
    await fireEvent.click(await screen.findByRole('radio', { name: /Request changes/ }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Request changes' })).toBeEnabled());
    await fireEvent.click(screen.getByRole('button', { name: 'Request changes' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_review').at(-1)).toMatchObject({ id: 15, revision: 6, review: { verdict: 'requestChanges' } }));
  });
});

describe('sections that share a heading in a long document', () => {
  it('lists every section of Install, Install, Install#2 and Reference, and a comment on the second Install stays on it when the comments are read again', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();
    knowledgeReviewDraft.add(17, 2, { section: 'Install', occurrence: 2 }, { side: 'new', startLine: 1, endLine: 1 }, 'Only on the second one.');

    const panel = await open('Install variants');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    const rows = await screen.findAllByRole('option');

    expect(rows).toHaveLength(4);
    expect(within(rows[1]).getByText('Install #2')).toBeInTheDocument();
    expect(within(rows[2]).getByText('Install#2')).toBeInTheDocument();
    expect(within(rows[0]).queryByText('1')).toBeNull();
    expect(within(rows[1]).getByText('1')).toBeInTheDocument();
    expect(within(rows[2]).queryByText('1')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Finish review (1)' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Comment' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_review').at(-1)).toMatchObject({ id: 17, review: { comments: [{ section: 'Install', occurrence: 2, body: 'Only on the second one.' }] } }));

    // The comments are read again from the server: the saved comment sits on the second Install only.
    await knowledgeReview.comments.refresh();
    const sections = await screen.findAllByRole('option');

    await waitFor(() => expect(within(sections[1]).getByText('1')).toBeInTheDocument());
    expect(within(sections[0]).queryByText('1')).toBeNull();
    expect(within(sections[2]).queryByText('1')).toBeNull();
    expect(screen.queryByText('Only on the second one.')).toBeNull();

    await fireEvent.click(sections[1]);

    expect(await screen.findByText('Only on the second one.')).toBeInTheDocument();
  });
});

describe('the history', () => {
  it('lists two events of the same kind at the same moment as two lines', async () => {
    const { store } = fakeKnowledge('drafts');
    const at = new Date(Date.now() - 3_600_000).toISOString();

    store.review.events.set('15', [
      { kind: 'other', by: 'Grace Hopper', at, comment: null },
      { kind: 'other', by: 'Grace Hopper', at, comment: null },
    ]);
    await show();

    const panel = await open('Billing API');

    await waitFor(() => expect(within(panel).getAllByText(/changed it/)).toHaveLength(2));
  });
});

describe('Show in folder after a download', () => {
  it('asks to reveal the saved file with the command made for downloads', async () => {
    const { invoke } = fakeKnowledge('drafts');
    await show();
    const panel = await open('Rollback runbook');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Download' }));
    await waitFor(() => expect(toasts.items.some((toast) => toast.action?.label === 'Show in folder')).toBe(true));

    toasts.items.find((toast) => toast.action?.label === 'Show in folder')?.action?.run();

    await waitFor(() => expect(calls(invoke, 'knowledge_reveal_download').at(-1)).toEqual({ path: '/home/demo/Downloads/rollback-runbook.md' }));
    expect(calls(invoke, 'reveal_path')).toHaveLength(0);
  });
});
