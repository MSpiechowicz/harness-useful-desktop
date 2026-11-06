import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { IpcFailure } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { GLOBAL_RETRY_MS, memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { PROJECT } from '../../../tests/chat-helpers';
import { guardsBackend } from '../../../tests/guards-backend';
import { startMockApp } from '../../../tests/mock-app';
import MemorySection from './MemorySection.svelte';

const READ_FAILED = 'Could not read what agents may save.';
const OLD_FILE = 'The saving settings file could not be updated. New chats and routines cannot start until it can be written.';
const UNREADABLE = new IpcFailure({ code: 'io', message: 'unreadable' });

beforeEach(async () => {
  await startMockApp(PROJECT);
  await memoryRemote.load();
  memoryChat.global = null;
  memoryChat.globalFailed = false;
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  toasts.items.splice(0);
  ecosystem.reset();
});

describe('Settings, Memory: what agents may save, when the saving file is not in order', () => {
  it('shows a spinner while it is read, then the cards', async () => {
    const backend = guardsBackend();
    const release = backend.hold();
    render(MemorySection, { id: 'memory' });

    expect(await screen.findByRole('status', { name: 'Reading what agents may save' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Notes' })).toBeNull();

    release();

    expect(await screen.findByRole('heading', { name: 'Notes' })).toBeInTheDocument();
    expect(screen.queryByRole('status', { name: 'Reading what agents may save' })).toBeNull();
  });

  it('says it could not read them instead of cards that cannot be trusted, and recovers by itself', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    const backend = guardsBackend();
    backend.guards.failReads(UNREADABLE);
    render(MemorySection, { id: 'memory' });

    expect(await screen.findByRole('alert')).toHaveTextContent(READ_FAILED);
    expect(screen.queryByRole('heading', { name: 'Notes' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Refresh' })).toBeNull();

    backend.guards.failReads(null);
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);

    expect(await screen.findByRole('heading', { name: 'Notes' })).toBeInTheDocument();
    expect(screen.queryByRole('alert')).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('warns once above the cards while the old file is in use, and clears when it could be written', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    const backend = guardsBackend();
    backend.guards.setMigrationPending(true);
    render(MemorySection, { id: 'memory' });

    const warning = (await screen.findByText(OLD_FILE)).closest('[role="status"]') as HTMLElement;
    const notes = await screen.findByRole('heading', { name: 'Notes' });

    expect(warning).toBeInTheDocument();
    expect(screen.getAllByText(OLD_FILE)).toHaveLength(1);
    expect(warning.compareDocumentPosition(notes) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    backend.guards.setMigrationPending(false);
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);

    await waitFor(() => expect(screen.queryByText(OLD_FILE)).toBeNull());
    expect(screen.getByRole('heading', { name: 'Notes' })).toBeInTheDocument();
  });

  it('keeps the cards, with what was last read, and an open dialog when a later read fails, and puts the alert above them', async () => {
    const backend = guardsBackend();
    render(MemorySection, { id: 'memory' });
    const notes = (await screen.findByRole('heading', { name: 'Notes' })).closest('section') as HTMLElement;
    await fireEvent.click(within(notes).getByRole('switch', { name: 'Notes: Save in every project' }));
    const dialog = await screen.findByRole('dialog', { name: 'Save notes in every project?' });

    backend.guards.failReads(UNREADABLE);
    await memoryChat.loadGlobal();

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent(READ_FAILED);
    expect(alert.compareDocumentPosition(screen.getByRole('heading', { name: 'Notes' })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Scripts' })).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'Save notes in every project?' })).toBe(dialog);

    // A change made in that state is written, and writing it shows what is current, so the alert goes.
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Turn on' }));

    await waitFor(() => expect(within(notes).getByRole('switch', { name: 'Notes: Save in every project' })).toBeChecked());
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  });

  it('keeps the cards when a later read fails, and a change refused in that state says why in a toast', async () => {
    const backend = guardsBackend();
    render(MemorySection, { id: 'memory' });
    const notes = (await screen.findByRole('heading', { name: 'Notes' })).closest('section') as HTMLElement;
    const pause = within(notes).getByRole('switch', { name: 'Notes: Pause saving' });
    backend.guards.failReads(UNREADABLE);
    await memoryChat.loadGlobal();
    await screen.findByRole('alert');
    backend.guards.failUpdates(new IpcFailure({ code: 'protocol', message: 'settings are busy' }));

    await fireEvent.click(pause);

    await waitFor(() => expect(toasts.items.map(({ description }) => description)).toEqual(['settings are busy']));
    expect(pause).not.toBeChecked();
    expect(screen.getByRole('alert')).toHaveTextContent(READ_FAILED);
    expect(screen.getByRole('heading', { name: 'Notes' })).toBeInTheDocument();
  });

  it('keeps a switch the user just changed when a slow read that started before the change comes back', async () => {
    const backend = guardsBackend();
    render(MemorySection, { id: 'memory' });
    const notes = (await screen.findByRole('heading', { name: 'Notes' })).closest('section') as HTMLElement;
    const pause = within(notes).getByRole('switch', { name: 'Notes: Pause saving' });
    const release = backend.hold();
    const slowRead = memoryChat.loadGlobal();

    await fireEvent.click(pause);
    await waitFor(() => expect(pause).toBeChecked());
    release();
    await slowRead;

    expect(pause).toBeChecked();
    expect(memoryChat.guards.notes.paused).toBe(true);
  });

  it.each([
    ['protocol', 'settings are busy'],
    ['invalidInput', 'the guards file is over 256 KiB'],
  ] as const)('says why a change was refused (%s) and the switch stays as it was', async (code, message) => {
    const backend = guardsBackend();
    render(MemorySection, { id: 'memory' });
    const notes = (await screen.findByRole('heading', { name: 'Notes' })).closest('section') as HTMLElement;
    const pause = within(notes).getByRole('switch', { name: 'Notes: Pause saving' });
    backend.guards.failUpdates(new IpcFailure({ code, message }));

    await fireEvent.click(pause);

    await waitFor(() => expect(toasts.items.map(({ title, description }) => ({ title, description }))).toEqual([{ title: 'Could not change the setting', description: message }]));
    expect(pause).not.toBeChecked();
    expect(memoryChat.guards.notes.paused).toBe(false);

    backend.guards.failUpdates(null);
    await fireEvent.click(pause);

    await waitFor(() => expect(pause).toBeChecked());
  });
});
