import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeHandlers, MockKnowledge } from '$lib/ipc/mock/knowledge-handlers';
import { IpcFailure } from '$lib/ipc/transport';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, spyOnInvoke } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { highlightPieces } from './knowledge-text';
import KnowledgeView from './KnowledgeView.svelte';

type Handler = (args: unknown) => unknown;

/** The preview's Harness Knowledge, with `wrap` able to change how one command answers (to fail it, for example). */
function fakeKnowledge(wrap: Record<string, (answer: Handler) => Handler> = {}) {
  const store = new MockKnowledge('ok');
  const handlers = knowledgeHandlers(store, () => 'ok', () => true) as unknown as Record<string, Handler>;
  const wrapped = Object.fromEntries(Object.entries(handlers).map(([name, handler]) => [name, wrap[name] ? wrap[name](handler) : handler]));

  return spyOnInvoke(wrapped);
}

/** Opens the document in the table row and answers its panel. */
async function open(title: string, row: number): Promise<HTMLElement> {
  render(KnowledgeView);
  await screen.findByRole('grid', { name: 'Documents' });
  await fireEvent.click(within(document.querySelector<HTMLElement>(`[data-row="${row}"]`) as HTMLElement).getByText(title));

  return screen.findByRole('region', { name: title });
}

const textReads = (invoke: ReturnType<typeof fakeKnowledge>) => (calls(invoke, 'knowledge_doc_get') as { chunkId: number | null }[]).filter((args) => args.chunkId !== null);

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  viewState.reset('knowledge');
  knowledgeDocs.list.sync(null, null);
  knowledgeDocs.detail.sync(null, null);
  knowledgeDocs.repos.sync(null, null);
  knowledgeChanges.reset();
  knowledgeStatus.value = null;
  projectScopes.sync(null, null);
});

afterEach(() => {
  nav.go('chat');
  vi.restoreAllMocks();
});

describe('the sections of an open document', () => {
  it('open to the text of the section, read once and only when opened, and every part of a long section is shown', async () => {
    const invoke = fakeKnowledge();
    const panel = await open('Billing API', 2);

    const install = await within(panel).findByRole('button', { name: 'Install' });

    expect(install).toHaveAttribute('aria-expanded', 'false');
    expect(textReads(invoke)).toHaveLength(0);

    await fireEvent.click(install);

    expect(install).toHaveAttribute('aria-expanded', 'true');
    expect(await within(panel).findByText(/describes the install in detail/)).toBeInTheDocument();
    expect(within(panel).getByText('harness doctor', { exact: false })).toBeInTheDocument();

    await fireEvent.click(install);
    await fireEvent.click(install);

    await within(panel).findByText(/describes the install in detail/);
    expect(textReads(invoke)).toHaveLength(1);

    // Configuration is cut into two chunks: one part at a time with a pager, each read only when shown.
    await fireEvent.click(within(panel).getByRole('button', { name: 'Configuration' }));

    expect(await within(panel).findByText('Part 1 of 2')).toBeInTheDocument();
    expect(within(panel).getByText(/Parts overlap a little/)).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Previous part' })).toBeDisabled();
    await waitFor(() => expect(textReads(invoke)).toHaveLength(2));

    await fireEvent.click(within(panel).getByRole('button', { name: 'Next part' }));

    expect(await within(panel).findByText('Part 2 of 2')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Next part' })).toBeDisabled();
    await waitFor(() => expect(textReads(invoke)).toHaveLength(3));
  });

  it('say why the text is missing and read it again on retry', async () => {
    let failing = true;
    fakeKnowledge({
      knowledge_doc_get: (answer) => (args) => {
        if ((args as { chunkId: number | null }).chunkId !== null && failing) {
          throw IpcFailure.from({ code: 'internal', message: 'the database is busy' });
        }

        return answer(args);
      },
    });
    const panel = await open('Billing API', 2);

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Overview' }));

    expect(await within(panel).findByRole('alert')).toHaveTextContent('the database is busy');

    failing = false;
    await fireEvent.click(within(panel).getByRole('button', { name: 'Try again' }));

    expect(await within(panel).findByText(/describes the overview in detail/)).toBeInTheDocument();
    expect(within(panel).queryByRole('alert')).toBeNull();
  });
});

describe('the matches of a search in an open document', () => {
  it('show nothing without a search, and with one list the passages of the open document only, marked and openable', async () => {
    const invoke = fakeKnowledge();
    const panel = await open('Billing API', 2);

    await within(panel).findByRole('button', { name: 'Overview' });

    expect(within(panel).queryByRole('heading', { name: 'Matches' })).toBeNull();
    expect(calls(invoke, 'knowledge_search')).toHaveLength(0);

    viewState.knowledgeQuery = 'billing';

    const heading = await within(panel).findByRole('heading', { name: 'Matches' });
    const matches = heading.closest('section') as HTMLElement;
    const hit = await within(matches).findByRole('button', { name: /Overview/ });

    expect(calls(invoke, 'knowledge_doc_matches')).toContainEqual({ id: 2, query: 'billing', limit: 20 });
    expect(calls(invoke, 'knowledge_search')).toHaveLength(0);
    expect(within(matches).getByText('6 matches')).toBeInTheDocument();
    expect(within(matches).getAllByRole('button')).toHaveLength(6);
    expect(hit.querySelector('mark')?.textContent).toBe('Billing');

    await fireEvent.click(hit);

    expect(await within(matches).findByText(/describes the overview in detail/)).toBeInTheDocument();

    viewState.knowledgeQuery = '';

    await waitFor(() => expect(within(panel).queryByRole('heading', { name: 'Matches' })).toBeNull());
  });

  it('fall back to the search of all documents for a server without doc-matches, and list the passages of the open document only', async () => {
    const invoke = fakeKnowledge({
      knowledge_status: (answer) => async (args) => {
        const status = (await answer(args)) as { features: object };

        return { ...status, features: { ...status.features, docMatches: false } };
      },
    });
    const panel = await open('Billing API', 2);

    await within(panel).findByRole('button', { name: 'Overview' });
    viewState.knowledgeQuery = 'billing';

    const heading = await within(panel).findByRole('heading', { name: 'Matches' });
    const matches = heading.closest('section') as HTMLElement;

    await within(matches).findByRole('button', { name: /Overview/ });

    expect(calls(invoke, 'knowledge_search')).toContainEqual({ query: 'billing', project: null, scope: 'all', limit: 20 });
    expect(calls(invoke, 'knowledge_doc_matches')).toHaveLength(0);
    expect(within(matches).getAllByRole('button')).toHaveLength(2);
    expect(within(matches).getByText('2 matches')).toBeInTheDocument();
  });

  it('say so when no passage of the open document matched closely', async () => {
    fakeKnowledge();
    const panel = await open('Billing API', 2);

    // Another document answers this search, so the open one has no passage in it.
    viewState.knowledgeQuery = 'release';

    expect(await within(panel).findByText('No passage matched closely. Open a section below to read it.')).toBeInTheDocument();
  });
});

describe('marking the words of a search', () => {
  it('marks words that start with a searched word, in any case, and keeps the rest as it was', () => {
    expect(highlightPieces('Install Version 1.2, then versions 10.', 'version 1')).toEqual([
      { text: 'Install ', match: false },
      { text: 'Version', match: true },
      { text: ' ', match: false },
      { text: '1', match: true },
      { text: '.2, then ', match: false },
      { text: 'versions', match: true },
      { text: ' ', match: false },
      { text: '10', match: true },
      { text: '.', match: false },
    ]);
  });

  it('treats the search as words and the text as text, so markup and pattern characters do nothing', () => {
    const pieces = highlightPieces('Use <b>bold</b> or (a|b) and a.b', '<b> (a|b) .*');

    expect(pieces.map((piece) => piece.text).join('')).toBe('Use <b>bold</b> or (a|b) and a.b');
    expect(pieces.filter((piece) => piece.match).map((piece) => piece.text)).toEqual(['b', 'bold', 'b', 'a', 'b', 'and', 'a', 'b']);
    expect(highlightPieces('a (b)', '(((')).toEqual([{ text: 'a (b)', match: false }]);
  });
});
