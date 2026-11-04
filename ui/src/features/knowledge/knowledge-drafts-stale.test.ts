import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { diffOf, numbered } from '$lib/ipc/mock/knowledge-review-diff';
import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
import { knowledgeReviewDraft } from '$lib/stores/knowledgeReviewDraft.svelte';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';
import { sectionCommentBlock } from './review-changes';

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

const NOT_ON_SCREEN = 'The changes you would decide on are not on screen yet.';

/**
 * The pending update of Billing API is row 15 (revision 6) until it is replaced by row 18, whose diff is never answered: what is on screen after that
 * can only be what was left of row 15. The user (Ada) commented on both at an earlier revision, so "Since your review" is offered.
 */
function replacedRow(): Record<string, (args: unknown) => unknown> {
  const sections = (heading: string) => numbered([diffOf(heading, { added: [`Only in ${heading}.`] })]);
  const comment = (id: number) => ({ id, body: 'Earlier remark', by: 'Ada Lovelace', at: '2026-11-01T10:00:00Z', revision: 3, outdated: true });

  return {
    knowledge_doc_compare: (args: unknown) => {
      const { id, since } = args as { id: number; since: number | null };

      return id === 15 ? { type: 'done', value: { live: 2, pending: 15, revision: 6, truncated: false, sections: sections(since === null ? 'Billing whole' : 'Billing since') } } : new Promise(() => {});
    },
    knowledge_doc_comments: (args: unknown) => [comment((args as { id: number }).id)],
  };
}

/** Another pending row takes the place of the one on screen: the same document, so the panel and the review stay open on it. */
async function replaceRow(store: { documents: { id: number | string }[] }): Promise<void> {
  const row = store.documents.find((doc) => doc.id === 15);

  if (row) {
    row.id = 18;
  }

  await knowledgeDocs.refresh();
}

describe('when the version on screen is replaced the diff of the one before is never on screen', () => {
  it('keeps Approve off in the panel with its reason, and sends nothing', async () => {
    const { invoke, store } = fakeKnowledge('drafts', false, replacedRow());
    await show();
    const panel = await open('Billing API');

    await waitFor(() => expect(within(panel).getByRole('button', { name: 'Approve' })).toBeEnabled());
    await replaceRow(store);

    await waitFor(() => expect(within(panel).getByText(NOT_ON_SCREEN)).toBeInTheDocument());
    expect(within(panel).getByRole('button', { name: 'Approve' })).toBeDisabled();
    expect(within(panel).queryByText(/Billing whole/)).toBeNull();
    await fireEvent.click(within(panel).getByRole('button', { name: 'Approve' }));
    expect(calls(invoke, 'knowledge_doc_transition')).toHaveLength(0);
  });

  it('shows the loading state in the review view and offers no verdict on the diff of the one before, in the comparison since the last review too', async () => {
    const { invoke, store } = fakeKnowledge('drafts', false, replacedRow());
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));
    await fireEvent.click(await screen.findByRole('radio', { name: 'Since your review' }));
    expect((await screen.findAllByText('Billing since')).length).toBeGreaterThan(0);

    await replaceRow(store);

    expect(await screen.findByRole('status', { name: 'Reading the changes' })).toBeInTheDocument();
    expect(screen.queryByText(/Billing since/)).toBeNull();
    expect(screen.queryByText(/Billing whole/)).toBeNull();

    await fireEvent.click(await screen.findByRole('button', { name: 'Finish review' }));
    await fireEvent.click(await screen.findByRole('radio', { name: /Approve/ }));

    const submit = screen.getAllByRole('button', { name: 'Approve' }).find((button) => button.closest('footer') !== null) as HTMLElement;

    expect(submit).toBeDisabled();
    expect(screen.getAllByText(NOT_ON_SCREEN).length).toBeGreaterThan(0);
    await fireEvent.click(submit);
    expect(calls(invoke, 'knowledge_doc_transition')).toHaveLength(0);
    expect(calls(invoke, 'knowledge_doc_review')).toHaveLength(0);
  });
});

describe('a section the server cannot keep a comment on', () => {
  it('says in grey that it takes no comments and offers no line to comment on', async () => {
    const huge = numbered([diffOf('Big', { added: ['A line.'] })]).map((section) => ({ ...section, occurrence: 1_000_001 }));
    const { invoke } = fakeKnowledge('drafts', false, { knowledge_doc_compare: () => ({ type: 'done', value: { live: 2, pending: 15, revision: 6, truncated: false, sections: huge } }) });
    await show();
    const panel = await open('Billing API');

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Review changes' }));

    expect(await screen.findByText("This section can't take comments.")).toBeInTheDocument();
    expect(screen.queryByText(/Click a line to comment/)).toBeNull();
    expect(calls(invoke, 'knowledge_doc_review')).toHaveLength(0);
  });
});

describe('which sections take a comment', () => {
  it('refuses a section beyond the number or the heading the server keeps a comment on, and no other', () => {
    const block = "This section can't take comments.";

    expect(sectionCommentBlock({ path: 'Install', occurrence: 1 })).toBeNull();
    expect(sectionCommentBlock({ path: 'a'.repeat(300), occurrence: 1_000_000 })).toBeNull();
    expect(sectionCommentBlock({ path: 'Install', occurrence: 1_000_001 })).toBe(block);
    expect(sectionCommentBlock({ path: 'a'.repeat(301), occurrence: 1 })).toBe(block);
    // A heading counts in characters, not in bytes.
    expect(sectionCommentBlock({ path: 'ł'.repeat(300), occurrence: 1 })).toBeNull();
  });
});
