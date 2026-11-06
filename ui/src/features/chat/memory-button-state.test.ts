import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { IpcFailure } from '$lib/ipc/transport';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { defaultGuards } from '$lib/stores/memory-chat';
import { GLOBAL_RETRY_MS, memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { chatSession, PROJECT } from '../../../tests/chat-helpers';
import { guardsBackend } from '../../../tests/guards-backend';
import { startMockApp } from '../../../tests/mock-app';
import MemoryButton from './MemoryButton.svelte';

const KEY = '11111111-2222-4333-8444-555555555555';
const GONE = '/nowhere/unknown';
const UNREADABLE = new IpcFailure({ code: 'io', message: 'unreadable' });
const READ_FAILED = 'Could not read what agents may save.';
const OLD_FILE = 'The saving settings file could not be updated. New chats and routines cannot start until it can be written.';
const attached: string[] = [];

/** A chat in `folder`, rendered, before anything about what every chat allows was read. */
function show(folder = PROJECT): () => void {
  const { session } = chatSession([], { project: folder, memoryGuards: true });
  memoryChat.attach(session.key, { key: KEY, guards: defaultGuards({ notes: false, scripts: false }) });
  attached.push(session.key);

  return render(MemoryButton, { session }).unmount;
}

async function open(): Promise<HTMLElement> {
  await fireEvent.click(await screen.findByRole('button', { name: /^Memory/ }));

  return screen.findByRole('dialog', { name: 'Saving to memory' });
}

/** Saving is turned on for the project, in the backend of the test. */
function turnProjectOn(backend: ReturnType<typeof guardsBackend>, folder = PROJECT): void {
  const handlers = backend.guards.handlers();

  for (const kind of ['notes', 'scripts'] as const) {
    handlers.memory_guards_update({ patch: { kind, addRepo: folder } });
  }
}

beforeEach(async () => {
  await startMockApp(PROJECT);
  harness.summaries = fixtures.harnesses;
  await memoryRemote.load();
  memoryChat.global = null;
  memoryChat.globalFailed = false;
});

afterEach(() => {
  attached.splice(0).forEach((key) => memoryChat.release(key));
  vi.useRealTimers();
  vi.restoreAllMocks();
  toasts.items.splice(0);
});

describe('the Memory button when what every chat allows cannot be read', () => {
  it('shows Memory disabled in red, and the popover says why without any switch to trust', async () => {
    const backend = guardsBackend();
    turnProjectOn(backend);
    backend.guards.failReads(UNREADABLE);

    show();
    const button = await screen.findByRole('button', { name: /^Memory disabled/ });
    const panel = await open();

    expect(button).toHaveClass('text-danger');
    expect(within(panel).getByRole('alert')).toHaveTextContent(READ_FAILED);
    expect(within(panel).queryAllByRole('switch')).toHaveLength(0);
    expect(within(panel).queryByRole('button', { name: 'Check again' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Pause' })).toBeNull();
  });

  it('keeps the switches, with what was last read, and the red Memory disabled when a later read fails, with the alert above them', async () => {
    const backend = guardsBackend();
    turnProjectOn(backend);
    show();
    const panel = await open();
    const project = await within(panel).findByRole('switch', { name: 'Notes: This project' });
    expect(project).toBeChecked();

    backend.guards.failReads(UNREADABLE);
    await memoryChat.loadGlobal();

    const alert = await within(panel).findByRole('alert');
    expect(alert).toHaveTextContent(READ_FAILED);
    expect(alert.compareDocumentPosition(project) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(panel).getByRole('switch', { name: 'Notes: This project' })).toBe(project);
    expect(project).toBeChecked();
    expect(screen.getByRole('button', { name: /^Memory disabled/ })).toHaveClass('text-danger');
  });

  it('writes a switch changed while a later read has failed, and says why in a toast when it is refused', async () => {
    const backend = guardsBackend();
    turnProjectOn(backend);
    show();
    const panel = await open();
    const project = await within(panel).findByRole('switch', { name: 'Notes: This project' });
    backend.guards.failReads(UNREADABLE);
    await memoryChat.loadGlobal();
    await within(panel).findByRole('alert');
    backend.guards.failUpdates(new IpcFailure({ code: 'protocol', message: 'settings are busy' }));

    await fireEvent.click(project);

    await waitFor(() => expect(toasts.items.map(({ description }) => description)).toEqual(['settings are busy']));
    expect(project).toBeChecked();
    expect(within(panel).getByRole('alert')).toHaveTextContent(READ_FAILED);

    backend.guards.failUpdates(null);
    await fireEvent.click(project);

    await waitFor(() => expect(project).not.toBeChecked());
    await waitFor(() => expect(within(panel).queryByRole('alert')).toBeNull());
  });

  it('reads again when the popover opens', async () => {
    const backend = guardsBackend();
    turnProjectOn(backend);
    backend.guards.failReads(UNREADABLE);
    show();
    await screen.findByRole('button', { name: /^Memory disabled/ });
    const reads = backend.count('memory_guards_get');
    backend.guards.failReads(null);

    const panel = await open();

    await waitFor(() => expect(within(panel).queryByRole('alert')).toBeNull());
    expect(backend.count('memory_guards_get')).toBeGreaterThan(reads);
    expect(within(panel).getByRole('switch', { name: 'Notes: This project' })).toBeChecked();
    expect(screen.getByRole('button', { name: /^Memory$/ })).not.toHaveClass('text-danger');
  });

  it('recovers by itself on a slow interval, and stops asking once it did', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    const backend = guardsBackend();
    turnProjectOn(backend);
    backend.guards.failReads(UNREADABLE);
    show();
    await screen.findByRole('button', { name: /^Memory disabled/ });
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);
    expect(screen.getByRole('button', { name: /^Memory disabled/ })).toHaveClass('text-danger');

    backend.guards.failReads(null);
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);

    expect(await screen.findByRole('button', { name: /^Memory$/ })).not.toHaveClass('text-danger');
    expect(vi.getTimerCount()).toBe(0);
  });

  it('keeps one timer going for several chats, and none after they are gone', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    const backend = guardsBackend();
    backend.guards.failReads(UNREADABLE);
    const stops = [show(), show()];
    await screen.findAllByRole('button', { name: /^Memory disabled/ });

    expect(vi.getTimerCount()).toBe(1);
    stops[0]();
    expect(vi.getTimerCount()).toBe(1);
    stops[1]();
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe('the Memory popover before what every chat allows was read', () => {
  it('shows no button until what every chat allows is known, so it never starts with the wrong chip', async () => {
    const backend = guardsBackend();
    turnProjectOn(backend);
    const release = backend.hold();
    show();

    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.queryByRole('button', { name: /^Memory/ })).toBeNull();

    release();

    const panel = await open();
    expect(await within(panel).findByRole('switch', { name: 'Notes: This project' })).toBeChecked();
  });
});

describe('the Memory button while the old saving file is still in use', () => {
  it('warns, says the chats stay off in place of the switches, and clears when the file could be written', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    const backend = guardsBackend();
    turnProjectOn(backend);
    backend.guards.setMigrationPending(true);
    show();

    const button = await screen.findByRole('button', { name: /^Memory disabled/ });
    const panel = await open();

    expect(button).toHaveClass('text-danger');
    expect(within(panel).getByRole('status')).toHaveTextContent(OLD_FILE);
    expect(within(panel).queryAllByRole('switch')).toHaveLength(0);

    backend.guards.setMigrationPending(false);
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);

    expect(await within(panel).findByRole('switch', { name: 'Notes: This project' })).toBeChecked();
    expect(within(panel).queryByText(OLD_FILE)).toBeNull();
    expect(screen.getByRole('button', { name: /^Memory$/ })).not.toHaveClass('text-danger');
  });
});

describe('the saving root of the chat folder in the Memory button', () => {
  it('is asked once for a folder that is gone, however long it is looked at', async () => {
    const backend = guardsBackend({ [GONE]: null });

    show(GONE);
    await screen.findByRole('button', { name: /^Memory/ });
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(backend.count('memory_saving_root')).toBe(1);
  });

  it('is asked again once per change of what every chat allows, and not otherwise', async () => {
    // A folder no test looked at before, so its root is not in the store's cache yet.
    const folder = '/home/demo/fresh/website';
    const backend = guardsBackend();
    turnProjectOn(backend, folder);
    show(folder);
    const panel = await open();
    const project = await within(panel).findByRole('switch', { name: 'Notes: This project' });
    await new Promise((resolve) => setTimeout(resolve, 50));
    const asked = backend.count('memory_saving_root');
    expect(asked).toBe(1);

    await fireEvent.click(project);
    await waitFor(() => expect(backend.count('memory_saving_root')).toBe(asked + 1));
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(backend.count('memory_saving_root')).toBe(asked + 1);
    expect(within(panel).getByRole('switch', { name: 'Notes: This project' })).not.toBeChecked();
  });
});
