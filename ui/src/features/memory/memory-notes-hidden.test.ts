import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { toasts } from '$lib/stores/toasts.svelte';

import { ALL, calls, fakeMemory, noteRow, PROJECT, registerMemoryHooks, showNotes } from './memory-test-support';

registerMemoryHooks();

const openNote = async (id: number, name: string) => {
  await fireEvent.click(noteRow(id) as HTMLElement);

  return screen.findByRole('region', { name });
};

/** Team notes the way a scope that saves on this computer shows them: read-only. */
const readOnlyTeam = () => sampleNotes().map((note) => ({ ...note, readOnly: note.place === 'team' }));

const TEAM_NOTE = 'Notes are read through the Memory CLI, never its database';
const LOCAL_NOTE = 'Tauri window stayed blank after a hot reload on Wayland';

describe('hiding team notes on this computer', () => {
  it('lets a read-only team note be hidden and nothing else', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, readOnlyTeam());
    await showNotes();

    const panel = await openNote(3, TEAM_NOTE);

    expect(within(panel).queryByRole('button', { name: 'Edit' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Delete' })).toBeNull();
    expect(within(panel).getByText(/read-only here/)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Hide on this computer' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_hide')).toEqual([{ id: 3, ...ALL, hidden: true }]));
    expect(await screen.findByRole('button', { name: 'Hidden on this computer (1)' })).toBeInTheDocument();
    expect(noteRow(3)).toBeNull();
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Hidden on this computer', description: TEAM_NOTE, tone: 'success' });
  });

  it('keeps Edit and Delete next to Hide for a team note of a scope on the team database, and offers no Hide for a note of this computer', async () => {
    fakeMemory();
    await showNotes();

    const team = await openNote(3, TEAM_NOTE);

    await waitFor(() => expect(within(team).getByRole('button', { name: 'Edit' })).toBeEnabled());
    expect(within(team).getByRole('button', { name: 'Delete' })).toBeInTheDocument();
    expect(within(team).getByRole('button', { name: 'Hide on this computer' })).toBeInTheDocument();

    const local = await openNote(4, LOCAL_NOTE);

    expect(within(local).getByRole('button', { name: 'Delete' })).toBeInTheDocument();
    expect(within(local).queryByRole('button', { name: 'Hide on this computer' })).toBeNull();
  });

  it('shows the hidden notes in their own filter, where they can only be shown again', async () => {
    const notes = sampleNotes().map((note) => (note.id === 5 ? { ...note, hidden: true } : note));
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await showNotes();

    expect(noteRow(5)).toBeNull();

    await fireEvent.click(await screen.findByRole('button', { name: 'Hidden on this computer (1)' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ hidden: true, offset: 0 }));
    expect(await screen.findByText('1 note')).toBeInTheDocument();

    const panel = await openNote(5, 'The release build fails on macOS when the notarization queue is slow');

    expect(calls(invoke, 'memory_note_get').at(-1)).toMatchObject({ id: 5, hidden: true });
    expect(within(panel).queryByRole('button', { name: 'Edit' })).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Delete' })).toBeNull();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Unhide' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_hide')).toEqual([{ id: 5, ...ALL, hidden: false }]));
    // The last hidden note is shown again: the filter has nothing left and the list is the full one.
    await waitFor(() => expect(screen.queryByRole('button', { name: /Hidden on this computer/ })).toBeNull());
    await waitFor(() => expect(noteRow(5)).not.toBeNull());
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Visible again', tone: 'success' });
  });

  it('has no switch to hidden notes while none are hidden', async () => {
    fakeMemory();
    await showNotes();

    expect(screen.queryByRole('button', { name: /Hidden on this computer/ })).toBeNull();
  });
});
