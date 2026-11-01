import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { documentOf } from '$lib/ipc/mock/knowledge-fixtures';
import { knowledgeHandlers, MockKnowledge, type KnowledgeScenario } from '$lib/ipc/mock/knowledge-handlers';
import { knowledgeImportHandlers } from '$lib/ipc/mock/knowledge-import-handlers';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, spyOnInvoke } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import KnowledgeView from './KnowledgeView.svelte';

/** A Harness Knowledge that keeps its documents in memory, as the preview does, and the spy that records what the view asked of it. */
function fakeKnowledge(scenario: KnowledgeScenario = 'ok') {
  const store = new MockKnowledge(scenario);
  const handlers = { ...knowledgeHandlers(store, () => scenario, () => true), ...knowledgeImportHandlers(store) } as unknown as Record<string, (args: unknown) => unknown>;
  // The real server answers with what it has now, not with the objects it keeps: a read after a change must not find the old answer changed under it.
  const answering = Object.fromEntries(Object.entries(handlers).map(([name, handler]) => [name, async (args: unknown) => structuredClone(await handler(args))]));
  const invoke = spyOnInvoke(answering);

  return { store, invoke };
}

async function show(): Promise<void> {
  render(KnowledgeView);
  await screen.findByRole('grid', { name: 'Documents' });
}

const titles = () => screen.queryAllByRole('gridcell').map((cell) => cell.textContent?.trim());

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  viewState.reset('knowledge');
  knowledgeDocs.list.sync(null, null);
  knowledgeDocs.detail.sync(null, null);
  knowledgeDocs.repos.sync(null, null);
  knowledgeHeld.list.sync(null, null);
  knowledgeHeld.detail.sync(null, null);
  knowledgeImport.reset();
  knowledgeChanges.reset();
  projectScopes.sync(null, null);
});

afterEach(() => {
  nav.go('chat');
  toasts.items = [];
  vi.restoreAllMocks();
});

describe('the documents table', () => {
  it('lists the documents with their format, where they apply and how many chunks they have, and tells a transcript from other text', async () => {
    fakeKnowledge();
    await show();

    const row = document.querySelector<HTMLElement>('[data-row="3"]') as HTMLElement;

    expect(within(row).getByText('Standup 28 October (transcript)')).toBeInTheDocument();
    expect(within(row).getByText('Transcript')).toBeInTheDocument();
    expect(within(row).getByText('Global')).toBeInTheDocument();
    expect(within(document.querySelector<HTMLElement>('[data-row="2"]') as HTMLElement).getByText('2 paths')).toBeInTheDocument();
  });

  it('shows what changed in the database by itself, with no refresh button', async () => {
    const { store } = fakeKnowledge();
    await show();
    const stop = knowledgeChanges.start();
    await waitFor(() => expect(knowledgeChanges.pulse).not.toBeNull());

    store.documents.push(documentOf({ id: 99, title: 'Added by an agent run', name: 'agent-run.md', format: 'markdown', kb: 2, chunks: 2, age: 0 }));
    store.changed();

    expect(await screen.findByText('Added by an agent run')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /refresh/i })).toBeNull();

    stop();
  });

  it('narrows the table by format on the server and goes back to the first page', async () => {
    const { invoke } = fakeKnowledge('bulk');
    await show();
    viewState.knowledgePage = 2;

    await fireEvent.click(screen.getByRole('button', { name: 'Format: All' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'PDF' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { format: 'pdf' }, offset: 0, limit: 50 }));
    expect(viewState.knowledgePage).toBe(0);
  });

  it('pages and sorts by the server, and says when nothing matches', async () => {
    const { invoke } = fakeKnowledge('bulk');
    await show();

    await fireEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ offset: 50 }));

    await fireEvent.click(screen.getByRole('button', { name: 'Title' }));
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { sort: 'title', order: 'asc' }, offset: 0 }));

    viewState.knowledgeQuery = 'zzzz-no-such';
    expect(await screen.findByText('No document matches your search.')).toBeInTheDocument();
  });

  it('offers to add the first documents when there are none', async () => {
    fakeKnowledge('empty');
    render(KnowledgeView);

    expect(await screen.findByText('Give your agents the documents they need')).toBeInTheDocument();
    // The switch stays, so the review queue is still one click away.
    expect(screen.getByRole('radio', { name: /Review/ })).toBeInTheDocument();
  });
});

describe('an open document', () => {
  it('shows the outline without counts and saves a path added with Enter, and Escape only puts the field away', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    await fireEvent.click(within(document.querySelector<HTMLElement>('[data-row="2"]') as HTMLElement).getByText('Billing API'));
    const panel = await screen.findByRole('region', { name: 'Billing API' });

    expect(await within(panel).findByText('Troubleshooting')).toBeInTheDocument();
    expect(within(panel).queryByText(/\d+ chunks/)).toBeNull();

    await fireEvent.click(within(panel).getByRole('button', { name: /Add a path to/ }));
    await fireEvent.keyDown(within(panel).getByRole('textbox', { name: /Path glob/ }), { key: 'Escape' });

    expect(within(panel).queryByRole('textbox', { name: /Path glob/ })).toBeNull();
    expect(screen.getByRole('region', { name: 'Billing API' })).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: /Add a path to/ }));
    const field = within(panel).getByRole('textbox', { name: /Path glob/ });
    await fireEvent.input(field, { target: { value: 'src/payments/**' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_links_set').at(-1)).toEqual({ id: 2, links: [{ repo: 'git:github.com/useful/harness-desktop', globs: ['src/billing/**', 'src/invoices/**', 'src/payments/**'] }] }));
    expect(await within(panel).findByText('Saved')).toBeInTheDocument();
  });

  it('links a repository chosen in the searchable menu of the repository filter, and unlinks it again', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    await fireEvent.click(within(document.querySelector<HTMLElement>('[data-row="6"]') as HTMLElement).getByText('Onboarding for new engineers'));
    const panel = await screen.findByRole('region', { name: 'Onboarding for new engineers' });
    expect(within(panel).getByText(/Applies to every repository/)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Link a repository' }));
    await fireEvent.input(await screen.findByRole('searchbox', { name: 'Search projects' }), { target: { value: 'web' } });
    expect(screen.queryByRole('menuitem', { name: /research/ })).toBeNull();
    await fireEvent.click(screen.getByRole('menuitem', { name: /useful\/website/ }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_links_set').at(-1)).toEqual({ id: 6, links: [{ repo: 'git:github.com/useful/website', globs: [] }] }));

    await fireEvent.click(await within(panel).findByRole('button', { name: 'Unlink useful/website' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_links_set').at(-1)).toEqual({ id: 6, links: [] }));
  });

  it('renames in place, and deletes only after the user confirmed', async () => {
    const { invoke, store } = fakeKnowledge();
    await show();

    await fireEvent.click(within(document.querySelector<HTMLElement>('[data-row="10"]') as HTMLElement).getByText('Glossary'));
    const panel = await screen.findByRole('region', { name: 'Glossary' });

    await fireEvent.click(within(panel).getByRole('button', { name: 'Rename document' }));
    const field = within(panel).getByRole('textbox', { name: 'Document title' });
    await fireEvent.input(field, { target: { value: 'Terms' } });
    await fireEvent.keyDown(field, { key: 'Enter' });
    await waitFor(() => expect(calls(invoke, 'knowledge_doc_rename').at(-1)).toEqual({ id: 10, title: 'Terms' }));
    expect(await screen.findByRole('region', { name: 'Terms' })).toBeInTheDocument();

    await fireEvent.click(within(await screen.findByRole('region', { name: 'Terms' })).getByRole('button', { name: 'Delete document' }));
    expect(calls(invoke, 'knowledge_docs_delete')).toHaveLength(0);

    await fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Delete document' }));
    await waitFor(() => expect(store.documents.some((doc) => doc.id === 10)).toBe(false));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Document deleted'));
  });
});

describe('the review queue', () => {
  it('reads a proposal before it is decided, approves it into the documents and drops its row', async () => {
    const { store, invoke } = fakeKnowledge();
    await show();

    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await screen.findByRole('grid', { name: 'Proposals waiting for review' });
    await fireEvent.click(screen.getByText('Install notes for Windows'));
    const panel = await screen.findByRole('region', { name: 'Install notes for Windows' });
    expect(await within(panel).findByText(/restart the shell so the new PATH applies/)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Approve' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_held_decide').at(-1)).toMatchObject({ decision: 'approve', revision: expect.any(String) }));
    await waitFor(() => expect(screen.queryByText('Install notes for Windows')).toBeNull());
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Added to Knowledge'));
    expect(store.documents.some((doc) => doc.title === 'Install notes for Windows' && doc.origin === 'agentApproved')).toBe(true);
  });

  it('asks before it throws a proposal away', async () => {
    const { invoke, store } = fakeKnowledge();
    await show();

    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await fireEvent.click(await screen.findByText('Local database reset'));
    const panel = await screen.findByRole('region', { name: 'Local database reset' });

    await fireEvent.click(within(panel).getByRole('button', { name: 'Reject' }));
    expect(await screen.findByText('Reject this proposal?')).toBeInTheDocument();
    expect(calls(invoke, 'knowledge_held_decide')).toHaveLength(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Keep it' }));
    expect(store.held).toHaveLength(3);
  });
});

describe('adding documents', () => {
  it('stages what was chosen, imports it with a result per file and lets the failed ones be looked at alone', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    await fireEvent.click(screen.getAllByRole('button', { name: 'Add documents' })[0]);
    await fireEvent.click(await screen.findByRole('button', { name: 'Choose files' }));
    expect(await screen.findByText('5 files')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    expect(await screen.findByText('Import finished')).toBeInTheDocument();

    expect(calls(invoke, 'knowledge_import').at(-1)).toMatchObject({ token: 'intake-5-0', options: { links: [], replaces: null } });
    expect(screen.getByText('the PDF has no text layer (a scan)')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: /^Failed/ }));
    const list = screen.getByRole('list', { name: 'Files of the import' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(1);
    expect(within(list).getByText('scan-of-contract.pdf')).toBeInTheDocument();
  });

  it('opens the window on a file dropped on the browser window', async () => {
    fakeKnowledge();
    await show();
    // A drag that carries files, as the browser reports it.
    const drag = (type: string) => {
      const event = new Event(type, { bubbles: true, cancelable: true });

      Object.defineProperty(event, 'dataTransfer', { value: { types: ['Files'] } });
      window.dispatchEvent(event);
    };

    drag('dragenter');
    expect(await screen.findByText('Drop to add documents')).toBeInTheDocument();

    drag('drop');
    expect(await screen.findByRole('button', { name: 'Start import' })).toBeInTheDocument();
  });
});

describe('when Harness Knowledge cannot be used', () => {
  it('offers the update inline for one that is too old', async () => {
    fakeKnowledge('outdated');
    render(KnowledgeView);

    expect(await screen.findByText('Harness Knowledge is out of date')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add documents' })).toBeNull();
  });

  it('says what a database lacks and where to fix it', async () => {
    fakeKnowledge('missing');
    render(KnowledgeView);

    expect(await screen.findByText('The database is missing something')).toBeInTheDocument();
    expect(screen.getByText(/pgvector extension is not installed/)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Open Knowledge settings' }));

    expect(nav.current).toBe('settings');
    expect(viewState.settingsTarget).toBe('knowledge');
  });
});
