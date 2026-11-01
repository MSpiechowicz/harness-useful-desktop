import { screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, pick } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, routedRemote, show } from './knowledge-view-support';

const BANNER = 'Remote knowledge is unreachable. Showing documents on this computer.';

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

describe('while the remote database is unreachable', () => {
  it('says so while the live counts say so, and stops saying so once they do not', async () => {
    const { store } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'));
    await show();

    expect(screen.queryByText(BANNER)).toBeNull();

    const stop = knowledgeChanges.start();
    await waitFor(() => expect(knowledgeChanges.pulse).not.toBeNull());

    knowledgeChanges.pulse = { ...(knowledgeChanges.pulse as NonNullable<typeof knowledgeChanges.pulse>), offline: true };
    expect(await screen.findByText(BANNER)).toBeInTheDocument();
    expect(screen.getByText('Writes to remote are paused until it is back.')).toBeInTheDocument();

    store.changed();
    await waitFor(() => expect(screen.queryByText(BANNER)).toBeNull());

    stop();
  });

  it('lists only the documents on this computer and offers the Kept in filter, which asks the server for one place', async () => {
    const { invoke } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'), true);
    await show();
    knowledgeChanges.start();

    expect(await screen.findByText(BANNER)).toBeInTheDocument();
    expect(await screen.findByText('Standup 28 October (transcript)')).toBeInTheDocument();
    expect(screen.queryByText('Architecture overview')).toBeNull();

    await pick('Kept in: All', /^Remote$/);

    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { place: 'remote' }, offset: 0 }));
    expect(viewState.knowledgePlace).toBe('remote');
    expect(await screen.findByText('No document matches these filters.')).toBeInTheDocument();
  });
});

describe('the Kept in filter', () => {
  it('narrows the table to one place on the server and goes back to the first page', async () => {
    const { invoke } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'));
    await show();
    viewState.knowledgePage = 1;

    await pick('Kept in: All', /^This computer$/);

    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { place: 'local' }, offset: 0 }));
    expect(viewState.knowledgePage).toBe(0);
    expect(await screen.findByText('Standup 28 October (transcript)')).toBeInTheDocument();
    expect(screen.queryByText('Architecture overview')).toBeNull();
  });

  it('is not offered with one place in use, and is let go of when the routing goes back to one', async () => {
    fakeKnowledge('ok', false, routedRemote('remote', 'remote'));
    viewState.knowledgePlace = 'local';
    await show();

    await waitFor(() => expect(viewState.knowledgePlace).toBeNull());
    expect(screen.queryByRole('button', { name: /Kept in/ })).toBeNull();
  });
});

describe('the Kept in filter on a server that cannot fall back to this computer', () => {
  it('is not offered, because the server would ignore the place', async () => {
    const { invoke } = fakeKnowledge('ok', true, {
      ...routedRemote('remoteLocal', 'remoteLocal'),
      knowledge_status: () => ({
        availability: { type: 'available' },
        features: { docs: true, import: true, embedder: true, search: true, recall: true, held: true, mcp: true, routing: true, docMatches: true, offlineFallback: false },
      }),
    });
    await show();

    expect(await screen.findByText('Architecture overview')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Kept in/ })).toBeNull();
    expect(calls(invoke, 'knowledge_docs_list').every((call) => (call as { filter: { place: unknown } }).filter.place === null)).toBe(true);
  });
});

describe('a remote document while the remote database is unreachable', () => {
  it('says it is unreachable in the app words in the panel', async () => {
    fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'), true);
    await show();

    // Opened from elsewhere (a search hit, a remembered selection): the row is not on the page.
    viewState.knowledgeDocId = 1;

    expect(await screen.findByText('This document is kept in the remote database, which is unreachable right now. Try again when it is back.')).toBeInTheDocument();
    expect(knowledgeDocs.detail.error?.code).toBe('unreachable');
  });
});

describe('a write to the remote database that cannot be reached', () => {
  it('says it is unreachable in the app words in a toast', async () => {
    const { store } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'));
    await show();
    await screen.findByText('Standup 28 October (transcript)');

    // The remote went away after the list was read: a change to one of its documents is refused.
    store.offline = true;

    const result = await knowledgeDocs.rename(1, 'New name');

    expect(result).toBeNull();
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Could not rename the document', description: 'This document is kept in the remote database, which is unreachable right now. Try again when it is back.', tone: 'danger' });
  });
});
