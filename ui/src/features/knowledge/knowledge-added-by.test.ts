import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AuthorRef } from '$lib/ipc/bindings/AuthorRef';
import { ADA, GRACE } from '$lib/ipc/mock/authors';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, pick } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';
import KnowledgeView from './KnowledgeView.svelte';

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

/** A person whose name is the word of a menu choice. */
const ALL: AuthorRef = { name: 'all', via: null };

/** Who added each of the first five documents: two Ada, one Grace, one called "all" and one unknown. */
const WHO: (AuthorRef | null)[] = [ADA, GRACE, ALL, null, ADA];

/** A Harness Knowledge that holds five documents with these authors. */
function withAuthors(who: (AuthorRef | null)[] = WHO, scenario: Parameters<typeof fakeKnowledge>[0] = 'ok') {
  const fake = fakeKnowledge(scenario);

  fake.store.documents = fake.store.documents.filter((doc) => doc.status === 'live' && doc.pendingOf == null).slice(0, who.length).map((doc, index) => ({ ...doc, addedBy: who[index] }));

  return { ...fake, ids: fake.store.documents.map((doc) => doc.id) };
}

const row = (id: number | string) => document.querySelector<HTMLElement>(`[data-row="${id}"]`);
const lastAsk = (invoke: { mock: { calls: unknown[][] } }) => calls(invoke, 'knowledge_docs_list').at(-1) as { filter: Record<string, unknown>; offset: number };

/** What the Added by menu offers, each choice with its count as the user reads it; the menu is closed again. */
async function choices(button: string): Promise<string[]> {
  await fireEvent.click(screen.getByRole('button', { name: button }));

  const open = await screen.findByRole('menu', { name: 'Added by' });
  const found = within(open).getAllByRole('menuitemradio').map((item) => item.textContent?.trim() ?? '');

  await fireEvent.keyDown(open, { key: 'Escape' });

  return found;
}

describe('the Added by filter of the documents', () => {
  it('lists everyone, each person with the count of what they added, and Unknown with its count', async () => {
    withAuthors();
    await show();

    // The people come in the server's order: the most documents first.
    expect(await choices('Added by: All')).toEqual(['All 5', 'Ada Lovelace 2', 'all 1', 'Grace Hopper 1', 'Unknown 1']);
  });

  it('has no Unknown choice while every document has a name', async () => {
    withAuthors([ADA, GRACE, GRACE]);
    await show();

    expect(await choices('Added by: All')).toEqual(['All 3', 'Grace Hopper 2', 'Ada Lovelace 1']);
  });

  it('narrows the list on the server to one person from the first page, and back with the clear button', async () => {
    const { invoke, ids } = withAuthors();
    await show();

    expect(lastAsk(invoke).filter).not.toHaveProperty('author');

    viewState.knowledgePage = 1;
    await pick('Added by: All', /^Grace Hopper/);

    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ filter: { author: { kind: 'name', name: 'Grace Hopper' } }, offset: 0 }));
    await waitFor(() => expect(row(ids[0])).toBeNull());
    expect(row(ids[1])).not.toBeNull();
    expect(viewState.knowledgePage).toBe(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Added by filter' }));

    await waitFor(() => expect(lastAsk(invoke).filter).not.toHaveProperty('author'));
    await waitFor(() => expect(row(ids[0])).not.toBeNull());
    expect(viewState.knowledgeAuthor).toBeNull();
  });

  it('keeps the other choices while one is chosen, and goes back to everyone with All', async () => {
    const { invoke } = withAuthors();
    await show();

    await pick('Added by: All', /^Ada Lovelace/);
    await waitFor(() => expect(lastAsk(invoke).filter).toMatchObject({ author: { kind: 'name', name: 'Ada Lovelace' } }));

    expect(await choices('Added by: Ada Lovelace')).toEqual(['All 5', 'Ada Lovelace 2', 'all 1', 'Grace Hopper 1', 'Unknown 1']);

    await pick('Added by: Ada Lovelace', /^All/);

    await waitFor(() => expect(lastAsk(invoke).filter).not.toHaveProperty('author'));
  });

  it('asks for the documents nobody is known to have added with Unknown', async () => {
    const { invoke, ids } = withAuthors();
    await show();

    await pick('Added by: All', /^Unknown/);

    await waitFor(() => expect(lastAsk(invoke).filter).toMatchObject({ author: { kind: 'unknown' } }));
    await waitFor(() => expect(row(ids[0])).toBeNull());
    expect(row(ids[3])).not.toBeNull();
  });

  it('tells a person called all from the All choice', async () => {
    const { invoke, ids } = withAuthors();
    await show();

    await pick('Added by: All', /^all/);

    await waitFor(() => expect(lastAsk(invoke).filter).toMatchObject({ author: { kind: 'name', name: 'all' } }));
    await waitFor(() => expect(row(ids[0])).toBeNull());
    expect(row(ids[2])).not.toBeNull();
    expect(screen.getByRole('button', { name: 'Added by: all' })).toBeInTheDocument();
  });

  it('keeps a chosen person who has nothing, says no document matches the filters, and clears from the menu button', async () => {
    const { ids } = withAuthors();
    await show();

    viewState.knowledgeAuthor = { kind: 'name', name: 'Nobody Atall' };

    expect(await screen.findByText('No document matches these filters.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Added by: Nobody Atall' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Added by filter' }));

    await waitFor(() => expect(row(ids[0])).not.toBeNull());
    expect(screen.queryByText('No document matches these filters.')).toBeNull();
  });

  it('sorts by the column from A to Z first, then the other way, asking the server', async () => {
    const { invoke } = withAuthors();
    await show();

    const header = screen.getByRole('columnheader', { name: 'Added by' });
    expect(header).toHaveAttribute('aria-sort', 'none');

    await fireEvent.click(within(header).getByRole('button', { name: 'Added by' }));
    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ filter: { sort: 'addedBy', order: 'asc' }, offset: 0 }));
    expect(header).toHaveAttribute('aria-sort', 'ascending');

    await fireEvent.click(within(header).getByRole('button', { name: 'Added by' }));
    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ filter: { sort: 'addedBy', order: 'desc' } }));
  });

  it('has no menu when every document is unknown and nothing narrows the list', async () => {
    const { ids } = withAuthors([null, null, null]);
    await show();

    expect(row(ids[0])).not.toBeNull();
    expect(screen.queryByRole('button', { name: /^Added by:/ })).toBeNull();
  });

  it('keeps the menu when a search leaves only documents nobody added', async () => {
    const { invoke, store, ids } = withAuthors();
    await show();

    // The document at position 3 has no known author.
    const title = store.documents[3].title;
    await fireEvent.input(screen.getByLabelText('Search documents'), { target: { value: title } });

    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ query: title }));
    await waitFor(() => expect(row(ids[0])).toBeNull());
    expect(await choices('Added by: All')).toEqual(['All 1', 'Unknown 1']);
  });

  it('forgets the choice when the view is reset', () => {
    viewState.knowledgeAuthor = { kind: 'unknown' };

    viewState.reset('knowledge');

    expect(viewState.knowledgeAuthor).toBeNull();
  });

  it('drops the choice and the sort for a Harness Knowledge that cannot filter by author, and offers neither', async () => {
    viewState.knowledgeAuthor = { kind: 'name', name: 'Ada Lovelace' };
    viewState.knowledgeSort = { key: 'addedBy', desc: false };
    const { invoke } = fakeKnowledge('older');
    await show();

    await waitFor(() => expect(knowledgeStatus.value?.features.docsAuthorFilter).toBe(false));
    await waitFor(() => expect(viewState.knowledgeAuthor).toBeNull());
    await waitFor(() => expect(viewState.knowledgeSort).toEqual({ key: 'updated', desc: true }));

    await waitFor(() => expect(lastAsk(invoke).filter).not.toHaveProperty('author'));
    expect(lastAsk(invoke).filter).toMatchObject({ sort: 'updated' });
    expect(screen.queryByRole('button', { name: /^Added by:/ })).toBeNull();
  });
});

describe('an empty project', () => {
  it('shows no Added by menu', async () => {
    fakeKnowledge('empty');
    render(KnowledgeView);

    expect(await screen.findByText('Give your agents the documents they need')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Added by:/ })).toBeNull();
  });
});
