import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
import { knowledgeReplace, routeNativeDrop } from '$lib/stores/knowledgeReplace.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, show } from './knowledge-view-support';

const FILE = { name: 'architecture-v2.md', path: '/home/demo/Documents/architecture-v2.md', bytes: 41_000 };
const one = { token: 'intake-1-0', files: [FILE], folders: [], skipped: [] };
const several = { token: 'intake-2-0', files: [FILE, { ...FILE, name: 'other.md', path: '/home/demo/Documents/other.md' }], folders: [], skipped: [] };

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

async function openDocument(row: number | string, name: string): Promise<HTMLElement> {
  await fireEvent.click(within(document.querySelector<HTMLElement>(`[data-row="${row}"]`) as HTMLElement).getByText(name));

  return screen.findByRole('region', { name });
}

describe('replacing an open document by a file', () => {
  it('asks first, then imports the chosen file in place of the document, keeps its links and updates the details without a refresh', async () => {
    const { invoke, store } = fakeKnowledge('ok', false, { knowledge_pick_files: () => one });
    await show();
    const panel = await openDocument(1, 'Architecture overview');

    await fireEvent.click(within(panel).getByRole('button', { name: 'Replace file…' }));

    // The native dialog only lets one file be chosen.
    await waitFor(() => expect(calls(invoke, 'knowledge_pick_files')).toEqual([{ multiple: false }]));

    const dialog = await screen.findByRole('dialog', { name: 'Replace this document?' });
    expect(within(dialog).getByText(/“architecture-v2\.md” \(\d+(\.\d)? KB\) takes its place/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Architecture overview/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Markdown · Updated by Grace Hopper/)).toBeInTheDocument();
    expect(calls(invoke, 'knowledge_import')).toHaveLength(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Replace document' }));

    await waitFor(() => expect(calls(invoke, 'knowledge_import').at(-1)).toMatchObject({ token: 'intake-1-0', options: { replaces: 1, links: store.documents[0].links } }));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Document replaced'));
    // The panel shows who changed it now, with no refresh button anywhere.
    await waitFor(() => expect(within(screen.getByRole('region', { name: 'Architecture overview' })).queryByText('Grace Hopper')).toBeNull());
    expect(screen.queryByRole('button', { name: /refresh/i })).toBeNull();
  });

  it('changes nothing when the user keeps the current document', async () => {
    const { invoke } = fakeKnowledge('ok', false, { knowledge_pick_files: () => one });
    await show();
    const panel = await openDocument(1, 'Architecture overview');

    await fireEvent.click(within(panel).getByRole('button', { name: 'Replace file…' }));
    await fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Keep current' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(calls(invoke, 'knowledge_import')).toHaveLength(0);
  });

  it('does not ask when the dialog is cancelled, and says why when several files were chosen', async () => {
    let picked: unknown = null;
    const { invoke } = fakeKnowledge('ok', false, { knowledge_pick_files: () => picked });
    await show();
    const panel = await openDocument(1, 'Architecture overview');

    await fireEvent.click(within(panel).getByRole('button', { name: 'Replace file…' }));
    await waitFor(() => expect(calls(invoke, 'knowledge_pick_files')).toHaveLength(1));
    expect(screen.queryByRole('dialog')).toBeNull();

    picked = several;
    await fireEvent.click(within(panel).getByRole('button', { name: 'Replace file…' }));

    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Choose one file'));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(calls(invoke, 'knowledge_import')).toHaveLength(0);
  });

  it('says so when the file has the text the document already has', async () => {
    fakeKnowledge('ok', false, {
      knowledge_pick_files: () => one,
      knowledge_import: (args: unknown) => {
        (args as { onEvent: (event: unknown) => void }).onEvent({ type: 'file', path: FILE.path, status: 'unchanged', reason: null, docId: 1, chunks: 18, embedding: 'done', warnings: [] });

        return { counts: { added: 0, updated: 0, unchanged: 1, skipped: 0, failed: 0 }, cancelled: false };
      },
    });
    await show();
    const panel = await openDocument(1, 'Architecture overview');

    await fireEvent.click(within(panel).getByRole('button', { name: 'Replace file…' }));
    await fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Replace document' }));

    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Nothing changed'));
  });
});

describe('routing a drop from the file manager', () => {
  const rect = { left: 1000, top: 100, right: 1500, bottom: 900 };
  const target = { rect: () => rect, drop: () => {} };

  it('replaces for one file inside the panel and imports for anything else', () => {
    expect(routeNativeDrop({ x: 1200, y: 400, items: 1, folder: false }, target)).toBe('replace');
    expect(routeNativeDrop({ x: 1000, y: 100, items: 1, folder: false }, target)).toBe('replace');
    expect(routeNativeDrop({ x: 999, y: 400, items: 1, folder: false }, target)).toBe('import');
    expect(routeNativeDrop({ x: 1500, y: 400, items: 1, folder: false }, target)).toBe('import');
    expect(routeNativeDrop({ x: 1200, y: 900, items: 1, folder: false }, target)).toBe('import');
    expect(routeNativeDrop({ x: 1200, y: 99, items: 1, folder: false }, target)).toBe('import');
    expect(routeNativeDrop({ x: 1200, y: 400, items: 2, folder: false }, target)).toBe('import');
    // A single folder inside the panel is an import: one document cannot be replaced by a folder.
    expect(routeNativeDrop({ x: 1200, y: 400, items: 1, folder: true }, target)).toBe('import');
  });

  it('imports when the pointer is unknown, no panel takes a drop or it is not on screen', () => {
    expect(routeNativeDrop(undefined, target)).toBe('import');
    expect(routeNativeDrop({ x: 1200, y: 400, items: 1, folder: false }, null)).toBe('import');
    expect(routeNativeDrop({ x: 1200, y: 400, items: 1, folder: false }, { ...target, rect: () => null })).toBe('import');
  });
});

describe('a file dragged in from the file manager over the open document', () => {
  type Listener = { onDrop: (event: unknown) => void };

  async function openWithListener(scenario: 'ok' | 'readonly' = 'ok') {
    const { invoke } = fakeKnowledge(scenario, false, {
      knowledge_drop_intake: () => one,
      knowledge_remote_get: () => null,
      ...(scenario === 'readonly' ? { knowledge_stats: () => ({ documents: 10, chunks: 0, embeddedChunks: 0, bytes: 0, globalDocuments: 0, repositories: 0, generation: 1, schema: 1, model: null, offline: false, remoteWritable: false }) } : {}),
    });
    await show();
    const panel = await openDocument(1, 'Architecture overview');
    vi.spyOn(panel, 'getBoundingClientRect').mockReturnValue({ left: 1000, top: 100, right: 1500, bottom: 900 } as DOMRect);
    // The panel tells the page where it is once it takes drops.
    if (scenario === 'readonly') {
      expect(await within(panel).findByText('Your role on Remote knowledge can only read')).toBeInTheDocument();
      expect(knowledgeReplace.target).toBeNull();
    } else {
      await waitFor(() => expect(knowledgeReplace.target).not.toBeNull());
    }
    const [{ onDrop }] = calls(invoke, 'drop_listen') as unknown as Listener[];

    return { invoke, onDrop, panel };
  }

  it('asks to replace it when one file is dropped on the panel, and does not open the import', async () => {
    const { invoke, onDrop, panel } = await openWithListener();

    onDrop({ type: 'hover', active: true, at: { x: 1200, y: 400, items: 1, folder: false } });
    expect(await within(panel).findByText('Drop to replace this document')).toBeInTheDocument();
    expect(screen.queryByText('Drop to add documents')).toBeNull();

    onDrop({ type: 'hover', active: false });
    onDrop({ type: 'dropped', images: [], files: [], skipped: [], at: { x: 1200, y: 400, items: 1, folder: false } });

    expect(await screen.findByRole('dialog', { name: 'Replace this document?' })).toBeInTheDocument();
    expect(calls(invoke, 'knowledge_drop_intake')).toHaveLength(1);
    expect(knowledgeImport.open).toBe(false);
  });

  it.each([
    ['outside the panel', { x: 300, y: 400, items: 1, folder: false }],
    ['many files', { x: 1200, y: 400, items: 3, folder: false }],
    ['one folder', { x: 1200, y: 400, items: 1, folder: true }],
  ])('goes to the import when it is %s', async (_, at) => {
    const { onDrop, panel } = await openWithListener();

    onDrop({ type: 'hover', active: true, at });
    expect(await screen.findByText('Drop to add documents')).toBeInTheDocument();
    expect(within(panel).queryByText('Drop to replace this document')).toBeNull();

    onDrop({ type: 'hover', active: false });
    onDrop({ type: 'dropped', images: [], files: [], skipped: [], at });

    await waitFor(() => expect(knowledgeImport.open).toBe(true));
    expect(screen.queryByRole('dialog', { name: 'Replace this document?' })).toBeNull();
  });

  it('goes to the import when the user may not change the document', async () => {
    const { onDrop, panel } = await openWithListener('readonly');
    const at = { x: 1200, y: 400, items: 1, folder: false };

    onDrop({ type: 'hover', active: true, at });
    expect(await screen.findByText('Drop to add documents')).toBeInTheDocument();
    expect(within(panel).queryByText('Drop to replace this document')).toBeNull();

    onDrop({ type: 'hover', active: false });
    onDrop({ type: 'dropped', images: [], files: [], skipped: [], at });

    await waitFor(() => expect(knowledgeImport.open).toBe(true));
  });
});

describe('a team database the user may only read', () => {
  const stats = { knowledge_stats: () => ({ documents: 10, chunks: 0, embeddedChunks: 0, bytes: 0, globalDocuments: 0, repositories: 0, generation: 1, schema: 1, model: null, offline: false, remoteWritable: false }) };

  it('turns Replace and Delete off and says why under them', async () => {
    const { invoke } = fakeKnowledge('readonly', false, stats);
    await show();
    const panel = await openDocument(1, 'Architecture overview');

    expect(await within(panel).findByText('Your role on Remote knowledge can only read')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Replace file…' })).toBeDisabled();
    expect(within(panel).getByRole('button', { name: 'Delete document' })).toBeDisabled();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Replace file…' }));
    expect(calls(invoke, 'knowledge_pick_files')).toHaveLength(0);
  });

  it('keeps the actions on while the role is not known', async () => {
    fakeKnowledge('ok', false, { knowledge_stats: () => ({ ...stats.knowledge_stats(), remoteWritable: null }) });
    await show();
    const panel = await openDocument(1, 'Architecture overview');

    expect(within(panel).getByRole('button', { name: 'Replace file…' })).toBeEnabled();
    expect(within(panel).getByRole('button', { name: 'Delete document' })).toBeEnabled();
    expect(within(panel).queryByText(/can only read/)).toBeNull();
  });

  it('only limits the documents kept in the remote place when both places are in use', async () => {
    fakeKnowledge('readonly', true, stats);
    await show();
    // Document 3 is kept on this computer (its id says so), document 1 in the team database.
    const local = await openDocument('local:3', 'Standup 28 October (transcript)');

    expect(within(local).getByRole('button', { name: 'Replace file…' })).toBeEnabled();

    const remote = await openDocument(1, 'Architecture overview');

    await waitFor(() => expect(within(remote).getByRole('button', { name: 'Replace file…' })).toBeDisabled());
  });
});

describe('a file dropped on the open panel', () => {
  const drop = (target: HTMLElement) => fireEvent(target, Object.assign(new Event('drop', { bubbles: true, cancelable: true }), { dataTransfer: { types: ['Files'] } }));
  const enter = (target: HTMLElement) => fireEvent(target, Object.assign(new Event('dragenter', { bubbles: true, cancelable: true }), { dataTransfer: { types: ['Files'] } }));

  it('says the drop replaces the document, and asks before it does', async () => {
    const { invoke } = fakeKnowledge('ok', false, { knowledge_drop_intake: () => one });
    await show();
    const panel = await openDocument(1, 'Architecture overview');

    await enter(panel);
    expect(await screen.findByText('Drop to replace this document')).toBeInTheDocument();
    expect(screen.queryByText('Drop to add documents')).toBeNull();

    await drop(panel);

    expect(await screen.findByRole('dialog', { name: 'Replace this document?' })).toBeInTheDocument();
    expect(calls(invoke, 'knowledge_drop_intake')).toHaveLength(1);
    expect(calls(invoke, 'knowledge_import')).toHaveLength(0);
  });

  it('leaves the drop to the page for a document the user may not change', async () => {
    fakeKnowledge('readonly', false, { knowledge_stats: () => ({ documents: 10, chunks: 0, embeddedChunks: 0, bytes: 0, globalDocuments: 0, repositories: 0, generation: 1, schema: 1, model: null, offline: false, remoteWritable: false }) });
    await show();
    const panel = await openDocument(1, 'Architecture overview');
    await within(panel).findByText(/can only read/);

    await enter(panel);

    expect(screen.queryByText('Drop to replace this document')).toBeNull();
  });
});
