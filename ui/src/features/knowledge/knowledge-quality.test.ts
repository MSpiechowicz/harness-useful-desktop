import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { ADA, GRACE_AGENT } from '$lib/ipc/mock/authors';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { documentOf } from '$lib/ipc/mock/knowledge-fixtures';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, pick } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  resetKnowledge();
});

afterEach(() => {
  nav.go('chat');
  toasts.items = [];
  vi.restoreAllMocks();
});

const row = (id: number) => document.querySelector<HTMLElement>(`[data-row="${id}"]`) as HTMLElement;

describe('how useful agents found the documents', () => {
  it('shows the times a document was found useful out of the times it was shown, and when last, and says nothing about a document nobody was shown', async () => {
    fakeKnowledge();
    await show();

    const architecture = row(1);
    expect(within(architecture).getByText('11 of 24')).toBeInTheDocument();
    expect(within(architecture).getByRole('progressbar', { name: 'Found useful in searches' })).toHaveAttribute('aria-valuenow', '11');
    expect(within(architecture).getByText('5 hours ago')).toBeInTheDocument();

    // Shown, never useful.
    expect(within(row(3)).getByText('0 of 7')).toBeInTheDocument();
    expect(within(row(3)).getByText('Never')).toBeInTheDocument();

    // Never shown: no count, no bar, no placeholder.
    expect(within(row(10)).queryByRole('progressbar')).toBeNull();
    expect(within(row(10)).queryByText('—')).toBeNull();
  });

  it('sorts by the server, biggest first, from the two columns and not from who added a document', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    expect(screen.queryByRole('button', { name: 'Added by' })).toBeNull();
    expect(screen.getByRole('columnheader', { name: 'Added by' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Useful in searches' }));
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { sort: 'useful', order: 'desc' }, offset: 0 }));

    await fireEvent.click(screen.getByRole('button', { name: 'Last useful' }));
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { sort: 'lastUseful', order: 'desc' } }));
  });

  it('narrows the table on the server to the documents found useful, or shown often and never useful, and back to all', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    await pick('Usefulness: All', /Useful at least once/);
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { useful: 'ever' }, offset: 0 }));

    await pick('Usefulness: Useful at least once', /Never useful/);
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { useful: 'never' } }));
    expect(await screen.findByText('Standup 28 October (transcript)')).toBeInTheDocument();
    expect(screen.queryByText('Architecture overview')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Usefulness filter' }));
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).not.toHaveProperty('filter.useful'));
    expect(await screen.findByText('Architecture overview')).toBeInTheDocument();
  });

  it('has no such columns, filter or sort for a Harness Knowledge that does not count it, and drops a choice of them', async () => {
    viewState.knowledgeUseful = 'ever';
    viewState.knowledgeSort = { key: 'useful', desc: true };
    const { invoke } = fakeKnowledge('older');
    await show();

    await waitFor(() => expect(viewState.knowledgeUseful).toBeNull());
    expect(screen.queryByRole('button', { name: 'Useful in searches' })).toBeNull();
    expect(screen.queryByRole('columnheader', { name: 'Last useful' })).toBeNull();
    expect(screen.queryByRole('columnheader', { name: 'Added by' })).toBeNull();
    expect(screen.queryByRole('button', { name: /^Usefulness/ })).toBeNull();

    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { sort: 'updated' } }));
    expect(calls(invoke, 'knowledge_docs_list').at(-1)).not.toHaveProperty('filter.useful');
  });
});

describe('who added a document', () => {
  it('names who added it in a column of its own, with the client an agent wrote through, and leaves the cell empty when no name was known', async () => {
    fakeKnowledge();
    await show();

    expect(within(row(1)).getByText('Ada Lovelace')).toBeInTheDocument();
    expect(within(row(8)).getByText(/Grace Hopper/)).toHaveTextContent('Grace Hopper via claude-code');
    expect(within(row(9)).queryByText('—')).toBeNull();
  });

  it('adds who added and who last changed a document to its details, and says Origin only where nobody is named', async () => {
    fakeKnowledge();
    await show();

    await fireEvent.click(within(row(1)).getByText('Architecture overview'));
    const panel = await screen.findByRole('region', { name: 'Architecture overview' });

    expect(within(panel).getByText('Added by').nextElementSibling).toHaveTextContent('Ada Lovelace');
    expect(within(panel).getByText('Updated by').nextElementSibling).toHaveTextContent('Grace Hopper');
    expect(within(panel).queryByText('Origin')).toBeNull();

    await fireEvent.click(within(row(9)).getByText('Design tokens'));
    const unnamed = await screen.findByRole('region', { name: 'Design tokens' });

    expect(within(unnamed).queryByText('Added by')).toBeNull();
    expect(within(unnamed).queryByText('Updated by')).toBeNull();
    expect(within(unnamed).getByText('Origin').nextElementSibling).toHaveTextContent('Added by you');
  });

  it('does not say Updated by when the same person added it and it never changed, and shows the client of an agent', async () => {
    const { store } = fakeKnowledge();
    const stamp = new Date(Date.now() - 3_600_000).toISOString();

    store.documents.push({ ...documentOf({ id: 40, title: 'Fresh from Ada', name: 'fresh.md', format: 'markdown', kb: 3, chunks: 2, age: 1, addedBy: ADA }), createdAt: stamp, updatedAt: stamp });
    store.documents.push({ ...documentOf({ id: 41, title: 'Fresh from an agent', name: 'agent.md', format: 'markdown', kb: 3, chunks: 2, age: 1, addedBy: GRACE_AGENT, updatedBy: ADA }), createdAt: stamp });
    await show();

    await fireEvent.click(within(row(40)).getByText('Fresh from Ada'));
    const fresh = await screen.findByRole('region', { name: 'Fresh from Ada' });

    expect(within(fresh).getByText('Added by').nextElementSibling).toHaveTextContent('Ada Lovelace');
    expect(within(fresh).queryByText('Updated by')).toBeNull();

    await fireEvent.click(within(row(41)).getByText('Fresh from an agent'));
    const agent = await screen.findByRole('region', { name: 'Fresh from an agent' });

    expect(within(agent).getByText('Added by').nextElementSibling).toHaveTextContent('Grace Hopper via claude-code');
    expect(within(agent).getByText('Updated by').nextElementSibling).toHaveTextContent('Ada Lovelace');
  });
});
