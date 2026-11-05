import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { documentOf } from '$lib/ipc/mock/knowledge-fixtures';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import KnowledgeView from './KnowledgeView.svelte';
import { fakeKnowledge, resetKnowledge, routedRemote, show } from './knowledge-view-support';

const titles = () => screen.queryAllByRole('gridcell').map((cell) => cell.textContent?.trim());

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

describe('the documents table', () => {
  it('lists the documents with their format, where they apply and how many chunks they have, and tells a transcript from other text', async () => {
    fakeKnowledge();
    await show();

    const row = document.querySelector<HTMLElement>('[data-row="3"]') as HTMLElement;

    expect(within(row).getByText('Standup 28 October (transcript)')).toBeInTheDocument();
    expect(within(row).getByText('Transcript')).toBeInTheDocument();
    expect(within(row).getByText('Global')).toBeInTheDocument();

    const narrowed = document.querySelector<HTMLElement>('[data-row="2"]') as HTMLElement;

    expect(screen.getByRole('columnheader', { name: 'Paths' })).toBeInTheDocument();
    expect(within(narrowed).getByText('2')).toBeInTheDocument();
    expect(within(narrowed).queryByText('2 paths')).not.toBeInTheDocument();
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

  it('sorts by the paths, most first', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    await fireEvent.click(await screen.findByRole('button', { name: 'Paths' }));
    await waitFor(() => expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { sort: 'paths', order: 'desc' } }));
    await waitFor(() => expect(document.querySelector('[data-row]')).toHaveAttribute('data-row', '2'));
  });

  it('keeps the column but offers no sort by paths on an older server, and leaves a sort it cannot do', async () => {
    viewState.knowledgeSort = { key: 'paths', desc: true };
    const { invoke } = fakeKnowledge('older');
    await show();

    expect(screen.getByRole('columnheader', { name: 'Paths' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Paths' })).not.toBeInTheDocument();
    await waitFor(() => expect(viewState.knowledgeSort).toEqual({ key: 'updated', desc: true }));
    expect(calls(invoke, 'knowledge_docs_list').at(-1)).toMatchObject({ filter: { sort: 'updated' } });
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
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Links saved'));
    expect(within(panel).queryByText('Saved')).toBeNull();
  });

  it('links a repository chosen in the searchable menu of the repository filter, and unlinks it again', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    await fireEvent.click(within(document.querySelector<HTMLElement>('[data-row="6"]') as HTMLElement).getByText('Onboarding for new engineers'));
    const panel = await screen.findByRole('region', { name: 'Onboarding for new engineers' });
    expect(within(panel).getByText(/Applies to every project/)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Link a project' }));
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
  it('keeps its filters when a scope leaves no proposal, so the scope can be undone', async () => {
    fakeKnowledge();
    await show();

    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await screen.findByRole('grid', { name: 'Proposals waiting for review' });
    viewState.heldScope = 'repo:git:github.com/nobody/nothing';

    expect(await screen.findByText('No proposal matches your search.')).toBeInTheDocument();
    expect(screen.queryByText('Nothing to review')).toBeNull();
    expect(screen.getByRole('radio', { name: /Documents/ })).toBeInTheDocument();
  });

  it('names the folder of a proposal in the table and the panel, with no tooltip that has the whole path', async () => {
    fakeKnowledge();
    await show();

    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await screen.findByRole('grid', { name: 'Proposals waiting for review' });

    const named = Array.from(document.querySelectorAll('[title="projects/harness-desktop"]'));
    expect(named.length).toBeGreaterThan(0);

    await fireEvent.click(screen.getByText('Install notes for Windows'));
    await screen.findByRole('region', { name: 'Install notes for Windows' });

    expect(Array.from(document.querySelectorAll('[title]')).filter((element) => element.getAttribute('title')?.includes('/home/dev'))).toEqual([]);
    expect(document.body.innerHTML).not.toContain('title="/home/dev');
  });

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

  it('throws a proposal away after a moment to undo it, with the reason for its agent', async () => {
    const { invoke, store } = fakeKnowledge();
    await show();

    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await fireEvent.click(await screen.findByText('Local database reset'));
    const panel = await screen.findByRole('region', { name: 'Local database reset' });

    await fireEvent.click(within(panel).getByRole('button', { name: 'Reject' }));
    await fireEvent.input(within(panel).getByRole('textbox', { name: /Reason for the agent/ }), { target: { value: 'Already documented' } });
    await fireEvent.click(within(panel).getByRole('button', { name: 'Reject' }));

    const toast = toasts.items.find((item) => item.title === 'Proposal rejected');

    expect(toast?.action?.label).toBe('Undo');
    expect(calls(invoke, 'knowledge_held_decide')).toHaveLength(0);

    toast?.action?.run();
    toasts.dismiss(toast?.id ?? 0);
    await new Promise((resolve) => setTimeout(resolve, 500));

    expect(calls(invoke, 'knowledge_held_decide')).toHaveLength(0);
    expect(store.held).toHaveLength(3);
  });

  it('asks the server to reject once the undo toast is gone', async () => {
    const { invoke } = fakeKnowledge();
    await show();

    await fireEvent.click(screen.getByRole('radio', { name: /Review/ }));
    await fireEvent.click(await screen.findByText('Local database reset'));
    const panel = await screen.findByRole('region', { name: 'Local database reset' });

    await fireEvent.click(within(panel).getByRole('button', { name: 'Reject' }));
    await fireEvent.input(within(panel).getByRole('textbox', { name: /Reason for the agent/ }), { target: { value: 'Already documented' } });
    await fireEvent.click(within(panel).getByRole('button', { name: 'Reject' }));

    toasts.dismiss(toasts.items.find((item) => item.title === 'Proposal rejected')?.id ?? 0);

    await waitFor(() => expect(calls(invoke, 'knowledge_held_decide').at(-1)).toMatchObject({ decision: 'reject', comment: 'Already documented' }));
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

async function stage(): Promise<void> {
  await fireEvent.click(screen.getAllByRole('button', { name: 'Add documents' })[0]);
  await fireEvent.click(await screen.findByRole('button', { name: 'Choose files' }));
  await screen.findByText('5 files');
}

describe('where an import is saved', () => {
  it('is chosen only while a kind of document reads Remote + this computer, and goes with the import', async () => {
    const { invoke } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remoteLocal'));
    await show();
    await stage();

    const group = await screen.findByRole('radiogroup', { name: 'Save to' });
    const [remote, local] = within(group).getAllByRole('radio');

    expect(remote).toBeChecked();
    expect(local).toBeEnabled();

    await fireEvent.click(local);
    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect(calls(invoke, 'knowledge_import').at(-1)).toMatchObject({ options: { place: 'local' } });
  });

  it('greys out a place the routing does not allow for the kind of the import, and sends the default for it', async () => {
    const { invoke } = fakeKnowledge('ok', true, routedRemote('remoteLocal', 'remote'));
    await show();
    await stage();

    // No links make the documents global, and global documents are kept remotely only.
    const group = await screen.findByRole('radiogroup', { name: 'Save to' });

    expect(within(group).getByRole('radio', { name: 'This computer' })).toBeDisabled();
    expect(within(group).getByRole('radio', { name: 'Remote' })).toBeChecked();

    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect(calls(invoke, 'knowledge_import').at(-1)).toMatchObject({ options: { place: 'remote' } });
  });

  it.each([
    ['remote', 'remote'],
    ['local', 'local'],
  ] as const)('is not asked for with one place for every kind (%s)', async (repository, global) => {
    const { invoke } = fakeKnowledge('ok', false, routedRemote(repository, global));
    await show();
    await stage();

    expect(screen.queryByRole('radiogroup', { name: 'Save to' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect((calls(invoke, 'knowledge_import').at(-1) as { options: object }).options).not.toHaveProperty('place');
  });

  it('is not asked for when the kinds are split between the places, the server decides by kind', async () => {
    fakeKnowledge('ok', true, routedRemote('remote', 'local'));
    await show();
    await stage();

    expect(screen.queryByRole('radiogroup', { name: 'Save to' })).toBeNull();
  });
});

describe('where each document is kept', () => {
  it('shows the place of every document while both are in use, and gives a local id back exactly as the server gave it', async () => {
    const { invoke } = fakeKnowledge('ok', true);
    await show();

    const row = (await screen.findByText('Standup 28 October (transcript)')).closest('[data-row]') as HTMLElement;

    expect(within(row).getByText('This computer')).toBeInTheDocument();
    expect(within(screen.getByText('Architecture overview').closest('[data-row]') as HTMLElement).getByText('Remote')).toBeInTheDocument();

    await fireEvent.click(within(row).getByText('Standup 28 October (transcript)'));
    const panel = await screen.findByRole('region', { name: 'Standup 28 October (transcript)' });

    await waitFor(() => expect(calls(invoke, 'knowledge_doc_get')).toContainEqual({ id: 'local:3', chunkId: null }));
    expect(within(panel).getAllByText('This computer').length).toBeGreaterThan(0);
  });

  it('shows no place with one place in use', async () => {
    fakeKnowledge();
    await show();

    await screen.findByText('Architecture overview');

    expect(screen.queryByText('This computer')).toBeNull();
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
