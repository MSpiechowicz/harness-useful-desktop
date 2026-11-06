import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { AuthorRef } from '$lib/ipc/bindings/AuthorRef';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { memoryRemoteHandlers } from '$lib/ipc/mock/memory-remote-handlers';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { addedByFact, updatedByFact } from '$lib/utils/author-format';

import { fakeMemory, noteRow, registerMemoryHooks, rowButton, showMemory, showNotes } from './memory-test-support';

registerMemoryHooks();

const ADA: AuthorRef = { name: 'Ada Lovelace', via: null };
const ADA_AGENT: AuthorRef = { name: 'Ada Lovelace', via: 'claude-code' };
const GRACE: AuthorRef = { name: 'Grace Hopper', via: null };

/** Makes the server's feature line `notes-authors` listed or not. */
function withNotesAuthors(invoke: ReturnType<typeof fakeMemory>, listed: boolean): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    if (name !== 'memory_remote_get') {
      return real(name as never, args);
    }

    return { ...(await real('memory_remote_get', undefined)), notesAuthorsSupported: listed };
  }) as typeof ipc.invoke);
}

describe('who added the notes', () => {
  it('adds the Added by column, with the client an agent wrote through and without one for the user', async () => {
    fakeMemory();
    await showNotes();

    expect(await screen.findByRole('columnheader', { name: 'Added by' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'Added by' })).toHaveAttribute('aria-sort', 'none');
    expect(within(noteRow(6) as HTMLElement).getByText('Ada Lovelace')).toHaveTextContent('Ada Lovelace via claude-code');
    expect(within(noteRow(3) as HTMLElement).getByText('Ada Lovelace')).toHaveTextContent(/^Ada Lovelace$/);
    expect(within(noteRow(4) as HTMLElement).queryByText(/Ada|Grace|via/)).toBeNull();
  });

  it('has no column for a server that does not tell who wrote a note, whatever a row carries', async () => {
    const invoke = fakeMemory();
    withNotesAuthors(invoke, false);
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesAuthorsSupported).toBe(false));

    expect(noteRow(3)).not.toBeNull();
    expect(screen.queryByRole('columnheader', { name: 'Added by' })).toBeNull();
    expect(within(noteRow(3) as HTMLElement).queryByText(/Ada/)).toBeNull();
  });

  it('lists who added and who last changed an open note, from the note it read', async () => {
    fakeMemory();
    await showNotes();

    await fireEvent.click(noteRow(3) as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'Notes are read through the Memory CLI, never its database' });

    for (const [label, value] of [['Saved from', 'claude-code'], ['Added by', 'Ada Lovelace'], ['Updated by', 'Grace Hopper']]) {
      expect(within(panel).getByText(label).nextElementSibling).toHaveTextContent(value);
    }
  });

  it('shows no Added by or Updated by row for a note without an author', async () => {
    fakeMemory();
    await showNotes();

    await fireEvent.click(noteRow(4) as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'Tauri window stayed blank after a hot reload on Wayland' });

    expect(within(panel).getByText('Saved from')).toBeInTheDocument();
    expect(within(panel).queryByText('Added by')).toBeNull();
    expect(within(panel).queryByText('Updated by')).toBeNull();
  });
});

describe('who added the scripts', () => {
  it('adds the sortable Added by column only for a server that tells who wrote a script', async () => {
    fakeMemory();
    await showMemory();

    expect(await screen.findByRole('columnheader', { name: 'Added by' })).toHaveAttribute('aria-sort', 'none');
    await waitFor(() => expect(within(rowButton('lint-changed') as HTMLElement).getByText('Ada Lovelace')).toHaveTextContent('Ada Lovelace via claude-code'));
    expect(within(rowButton('ci-status') as HTMLElement).getByText('Ada Lovelace')).toHaveTextContent(/^Ada Lovelace$/);
    expect(within(rowButton('open-prs') as HTMLElement).queryByText(/Ada|Grace|via/)).toBeNull();
  });

  it('has no column for a server without notes-authors', async () => {
    const invoke = fakeMemory();
    withNotesAuthors(invoke, false);
    await showMemory();
    await waitFor(() => expect(memoryRemote.value?.notesAuthorsSupported).toBe(false));

    expect(screen.queryByRole('columnheader', { name: 'Added by' })).toBeNull();
  });

  it('lists who added a script and who changed it in the Details', async () => {
    fakeMemory();
    await showMemory();

    await fireEvent.click(rowButton('ci-status') as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'ci-status' });

    expect(within(panel).getByText('Added by').nextElementSibling).toHaveTextContent('Ada Lovelace');
    expect(within(panel).getByText('Updated by').nextElementSibling).toHaveTextContent('Grace Hopper');
  });
});

describe('the Details rows of authors', () => {
  const moment = '2026-10-01T10:00:00Z';
  const later = '2026-10-02T10:00:00Z';

  it('leaves out Updated by for the same author who never changed it again', () => {
    const item = { createdAt: moment, updatedAt: moment, addedBy: ADA_AGENT, updatedBy: { ...ADA_AGENT } };

    expect(addedByFact(item).map((row) => row.value)).toEqual(['Ada Lovelace via claude-code']);
    expect(updatedByFact(item)).toEqual([]);
  });

  it('keeps Updated by once the same author changed it again, or another one did', () => {
    expect(updatedByFact({ createdAt: moment, updatedAt: later, addedBy: ADA, updatedBy: ADA }).map((row) => row.value)).toEqual(['Ada Lovelace']);
    expect(updatedByFact({ createdAt: moment, updatedAt: moment, addedBy: ADA, updatedBy: GRACE }).map((row) => row.value)).toEqual(['Grace Hopper']);
    expect(updatedByFact({ createdAt: moment, updatedAt: moment, addedBy: ADA, updatedBy: ADA_AGENT }).map((row) => row.value)).toEqual(['Ada Lovelace via claude-code']);
  });

  it('has no row for an author that is not known', () => {
    expect(addedByFact({ addedBy: null })).toEqual([]);
    expect(addedByFact({})).toEqual([]);
    expect(updatedByFact({ createdAt: moment, updatedAt: later, addedBy: ADA, updatedBy: null })).toEqual([]);
    expect(updatedByFact({ createdAt: moment, updatedAt: later })).toEqual([]);
  });
});

describe('the preview of a Harness Memory without notes-authors', () => {
  it('lists notes-authors like a current Harness Memory, and not for ?authors=old', async () => {
    expect((await memoryRemoteHandlers().memory_remote_get(undefined)).notesAuthorsSupported).toBe(true);

    window.history.replaceState(null, '', '/?authors=old');

    expect((await memoryRemoteHandlers().memory_remote_get(undefined)).notesAuthorsSupported).toBe(false);
    window.history.replaceState(null, '', '/');
  });
});
