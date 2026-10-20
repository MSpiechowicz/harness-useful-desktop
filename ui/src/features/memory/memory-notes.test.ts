import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { ALL, calls, fakeMemory, noteRow, pickFilter, PROJECT, registerMemoryHooks, showNotes } from './memory-test-support';
import NotesView from './NotesView.svelte';
import ScriptsView from './ScriptsView.svelte';

registerMemoryHooks();

const openNote = async (id: number, name: string) => {
  await fireEvent.click(noteRow(id) as HTMLElement);

  return screen.findByRole('region', { name });
};

describe('notes table', () => {
  it('opens on the notes and lists a page of them with kind, scope, place and age, asked for by project', async () => {
    const invoke = fakeMemory();
    await showNotes();

    expect(calls(invoke, 'memory_notes_list')[0]).toEqual({ ...ALL, offset: 0, limit: 50, sort: 'updated', order: 'desc' });

    const decision = within(noteRow(3) as HTMLElement);
    expect(decision.getByText('Notes are read through the Memory CLI, never its database')).toBeInTheDocument();
    expect(decision.getByText('Decision')).toBeInTheDocument();
    expect(decision.getByText('github.com/useful/harness-desktop')).toBeInTheDocument();
    expect(decision.getByText('Remote')).toBeInTheDocument();
    expect(within(noteRow(9) as HTMLElement).getByText('Global')).toBeInTheDocument();
  });

  it('writes a hidden character of a title out instead of rendering it', async () => {
    const notes = sampleNotes().map((note) => (note.id === 4 ? { ...note, title: 'Fix‮ gnirts' } : note));
    fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await showNotes();

    expect(within(noteRow(4) as HTMLElement).getByText('Fix⟨U+202E⟩ gnirts')).toBeInTheDocument();
  });

  it('asks the server for one kind when it is chosen, and keeps the counts of the others', async () => {
    const invoke = fakeMemory();
    await showNotes();

    await pickFilter('Kind: All', /^Decision/);

    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ kind: 'decision', offset: 0 }));
    await waitFor(() => expect(noteRow(4)).toBeNull());
    expect(noteRow(3)).not.toBeNull();
    expect(screen.getByRole('button', { name: 'Kind: Decision' })).toBeInTheDocument();
    expect(screen.getByText('11 notes')).toBeInTheDocument();
  });

  it('turns the pages and says where the user is', async () => {
    const invoke = fakeMemory();
    await showNotes();

    expect(screen.getByText('1–50 of 70')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(await screen.findByText('51–70 of 70')).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ offset: 50, limit: 50 });
    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();

    await fireEvent.click(screen.getByRole('button', { name: 'Page 1' }));

    expect(await screen.findByText('1–50 of 70')).toBeInTheDocument();
  });

  it('searches on the server once typing pauses, and starts again from the first page', async () => {
    const invoke = fakeMemory();
    viewState.notesPage = 1;
    await showNotes();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search notes' }), { target: { value: 'dmabuf' } });

    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ query: 'dmabuf', offset: 0 }));
    await waitFor(() => expect(noteRow(3)).toBeNull());
    expect(noteRow(4)).not.toBeNull();
    expect(screen.getByText('1 note')).toBeInTheDocument();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search notes' }), { target: { value: 'zzz-nothing' } });

    expect(await screen.findByText('No note matches that search.')).toBeInTheDocument();
  });

  it('sorts by any column when its header is clicked, texts from A and times from the newest, and starts at the first page', async () => {
    const invoke = fakeMemory();
    await showNotes();
    const lastAsk = () => calls(invoke, 'memory_notes_list').at(-1);

    expect(screen.getByRole('columnheader', { name: 'Updated' })).toHaveAttribute('aria-sort', 'descending');

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(lastAsk()).toMatchObject({ offset: 50 }));

    for (const [header, sort] of [['Title', 'title'], ['Kind', 'kind'], ['Scope', 'scope'], ['Kept in', 'place']] as const) {
      await fireEvent.click(screen.getByRole('button', { name: header }));

      await waitFor(() => expect(lastAsk()).toMatchObject({ sort, order: 'asc', offset: 0 }));
      expect(screen.getByRole('columnheader', { name: header })).toHaveAttribute('aria-sort', 'ascending');
    }

    await fireEvent.click(screen.getByRole('button', { name: 'Updated' }));
    await waitFor(() => expect(lastAsk()).toMatchObject({ sort: 'updated', order: 'desc' }));

    await fireEvent.click(screen.getByRole('button', { name: 'Updated' }));
    await waitFor(() => expect(lastAsk()).toMatchObject({ sort: 'updated', order: 'asc' }));
  });
});

describe('note panel', () => {
  it('opens beside the table with the content, tags and entities, links, actions and details', async () => {
    const invoke = fakeMemory();
    await showNotes();

    expect(screen.queryByRole('region', { name: 'Tauri window stayed blank after a hot reload on Wayland' })).toBeNull();
    const panel = await openNote(4, 'Tauri window stayed blank after a hot reload on Wayland');

    expect(within(panel).getByText('Fix · github.com/useful/harness-desktop')).toBeInTheDocument();
    expect(await within(panel).findByText(/WebKitGTK renders into a DMA buffer/)).toBeInTheDocument();
    expect(calls(invoke, 'memory_note_get')[0]).toEqual({ id: 4, ...ALL });
    for (const heading of ['Content', 'Tags & entities', 'Linked notes', 'Actions', 'Details']) {
      expect(within(panel).getByRole('heading', { level: 3, name: heading })).toBeInTheDocument();
    }

    expect(within(panel).getByText('linux')).toBeInTheDocument();
    expect(within(panel).getByText('WEBKIT_DISABLE_DMABUF_RENDERER')).toBeInTheDocument();

    for (const [label, value] of [['Scope', 'github.com/useful/harness-desktop'], ['Kept in', 'This computer'], ['Written by', 'claude-code'], ['Folder', '~/projects/harness-desktop'], ['Revision', '1'], ['Recalled', '7 times']]) {
      expect(within(panel).getByText(label).nextElementSibling).toHaveTextContent(value);
    }
    expect(within(panel).getByText('Created')).toBeInTheDocument();
    expect(within(panel).getByText('Updated')).toBeInTheDocument();
  });

  it('keeps the line breaks of the body and writes hidden characters out with a warning', async () => {
    const notes = sampleNotes().map((note) => (note.id === 4 ? { ...note, body: 'first line\nsecond‮ line​' } : note));
    fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await showNotes();

    const panel = await openNote(4, 'Tauri window stayed blank after a hot reload on Wayland');

    expect(await within(panel).findByText('This note contains 2 invisible characters, shown below in red.')).toBeInTheDocument();
    expect(Array.from(panel.querySelectorAll('[data-hidden-char]')).map((node) => node.textContent)).toEqual(['⟨U+202E⟩', '⟨U+200B⟩']);
    const body = panel.querySelector('p.whitespace-pre-wrap');
    expect(body?.textContent).toBe('first line\nsecond⟨U+202E⟩ line⟨U+200B⟩');
  });

  it('lists the linked notes with how they relate, and a click on one opens that note', async () => {
    const invoke = fakeMemory();
    await showNotes();
    const panel = await openNote(4, 'Tauri window stayed blank after a hot reload on Wayland');

    const links = await within(panel).findAllByRole('button', { name: /The release build fails on macOS/ });
    expect(links).toHaveLength(2);
    expect(within(links[0]).getByText('related to')).toBeInTheDocument();
    expect(within(links[1]).getByText('caused by')).toBeInTheDocument();
    expect(within(links[0]).getByText('Problem')).toBeInTheDocument();

    await fireEvent.click(links[0]);

    const next = await screen.findByRole('region', { name: 'The release build fails on macOS when the notarization queue is slow' });
    expect(await within(next).findByText(/notarytool waits 10 minutes/)).toBeInTheDocument();
    expect(calls(invoke, 'memory_note_get').at(-1)).toEqual({ id: 5, ...ALL });
    expect(viewState.noteId).toBe(5);
  });

  it('opens a note that is not on the page shown when a link leads to it', async () => {
    fakeMemory();
    viewState.notesKind = 'decision';
    await showNotes();
    const panel = await openNote(3, 'Notes are read through the Memory CLI, never its database');

    await fireEvent.click(await within(panel).findByRole('button', { name: /Where the notes view keeps its state/ }));

    const next = await screen.findByRole('region', { name: 'Where the notes view keeps its state between visits' });
    expect(within(next).getByText('Location · github.com/useful/harness-desktop')).toBeInTheDocument();
    expect(noteRow(8)).toBeNull();
  });

  it('browses with the arrow keys and closes on Escape with the focus back on the row', async () => {
    fakeMemory();
    await showNotes();
    const first = noteRow(3) as HTMLElement;
    first.focus();

    await fireEvent.keyDown(first, { key: 'Enter' });
    await screen.findByRole('region', { name: 'Notes are read through the Memory CLI, never its database' });

    await fireEvent.keyDown(first, { key: 'ArrowDown' });

    expect(await screen.findByRole('region', { name: 'Tauri window stayed blank after a hot reload on Wayland' })).toBeInTheDocument();
    expect(document.activeElement).toBe(noteRow(4));

    await fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('region', { name: 'Tauri window stayed blank after a hot reload on Wayland' })).toBeNull());
    expect(document.activeElement).toBe(noteRow(4));
  });

  it('shows the placeholder of a note that has no tags, entities or links', async () => {
    const notes = sampleNotes().map((note) => (note.id === 9 ? { ...note, tags: [] } : note));
    fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await showNotes();

    const panel = await openNote(9, 'Run the mock backend with ?notes=empty to see the empty state');

    expect(within(panel).getByText('No tags or entities')).toBeInTheDocument();
    expect(await within(panel).findByText('Linked notes')).toBeInTheDocument();
  });
});

describe('deleting a note', () => {
  async function askToDelete(id: number, name: string): Promise<void> {
    const panel = await openNote(id, name);

    await fireEvent.click(within(panel).getByRole('button', { name: 'Delete' }));
  }

  it('asks first, names where the note lives, then deletes it, closes the panel and reads the page again', async () => {
    const invoke = fakeMemory();
    await showNotes();

    await askToDelete(3, 'Notes are read through the Memory CLI, never its database');

    expect(await screen.findByText('Delete this note?')).toBeInTheDocument();
    expect(screen.getByText('This note is kept in the remote database. Deleting it removes it for everyone.')).toBeInTheDocument();
    expect(calls(invoke, 'memory_note_delete')).toHaveLength(0);

    await fireEvent.click(await screen.findByRole('button', { name: 'Delete note' }));

    await waitFor(() => expect(noteRow(3)).toBeNull());
    expect(calls(invoke, 'memory_note_delete')).toEqual([{ id: 3, ...ALL }]);
    expect(calls(invoke, 'memory_notes_list').length).toBeGreaterThan(1);
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Notes are read through the Memory CLI, never its database' })).toBeNull());
    expect(viewState.noteId).toBeNull();
  });

  it('keeps the note when the confirmation is cancelled, and does not mention the team for a local one', async () => {
    const invoke = fakeMemory();
    await showNotes();

    await askToDelete(4, 'Tauri window stayed blank after a hot reload on Wayland');
    expect(await screen.findByText('Delete this note?')).toBeInTheDocument();
    expect(screen.queryByText(/kept in the remote database/)).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Keep it' }));

    expect(calls(invoke, 'memory_note_delete')).toHaveLength(0);
    expect(noteRow(4)).not.toBeNull();
  });

  it('says so when the delete fails and keeps the row', async () => {
    const invoke = fakeMemory();
    await showNotes();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    invoke.mockImplementation(((name: CommandName, args: never) =>
      name === 'memory_note_delete' ? Promise.reject(IpcFailure.from({ code: 'internal', message: 'database is locked' })) : real(name as never, args)) as typeof ipc.invoke);

    await askToDelete(4, 'Tauri window stayed blank after a hot reload on Wayland');
    await fireEvent.click(await screen.findByRole('button', { name: 'Delete note' }));

    await waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ title: 'Could not delete the note', description: 'database is locked' }));
    expect(noteRow(4)).not.toBeNull();
  });
});

describe('the Notes and Scripts views', () => {
  it('are two views, each with its own title and no switch between lists', async () => {
    fakeMemory();
    nav.go('notes');
    const notes = render(NotesView);
    await screen.findByRole('grid', { name: 'Notes' });

    expect(screen.getByRole('heading', { level: 1, name: 'Notes' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Memory section/ })).toBeNull();
    expect(screen.queryByRole('grid', { name: 'Scripts' })).toBeNull();
    notes.unmount();

    nav.go('scripts');
    render(ScriptsView);
    await screen.findByRole('grid', { name: 'Scripts' });

    expect(screen.getByRole('heading', { level: 1, name: 'Scripts' })).toBeInTheDocument();
    expect(screen.queryByRole('grid', { name: 'Notes' })).toBeNull();
  });

  it('keep their search in their own header while the other view is still fading out', async () => {
    fakeMemory();
    nav.go('scripts');
    const leaving = render(ScriptsView);
    await screen.findByRole('grid', { name: 'Scripts' });

    nav.go('notes');
    render(NotesView);
    await screen.findByRole('grid', { name: 'Notes' });
    leaving.unmount();

    expect(screen.getByRole('searchbox', { name: 'Search notes' })).toBeInTheDocument();
  });

  it('keep the search of each list when the view is left and opened again', async () => {
    fakeMemory();
    const first = render(NotesView);
    await screen.findByRole('grid', { name: 'Notes' });
    viewState.notesQuery = 'wayland';
    first.unmount();

    render(NotesView);

    expect(await screen.findByRole('searchbox', { name: 'Search notes' })).toHaveValue('wayland');
  });
});

describe('when there is nothing to list', () => {
  it('says that agents have not written notes yet', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, []);
    nav.go('notes');
    render(NotesView);

    expect(await screen.findByText('No notes yet')).toBeInTheDocument();
    expect(screen.getByText(/Agents haven't written notes for this project yet/)).toBeInTheDocument();
    expect(screen.queryByRole('searchbox', { name: 'Search notes' })).toBeNull();
  });

  it('offers to update when this Harness Memory is too old for the notes', async () => {
    fakeMemory({ kind: 'outdated' });
    nav.go('notes');
    render(NotesView);

    expect(await screen.findByText('Your memory needs an update')).toBeInTheDocument();
  });

  it('shows what went wrong and tries again on request', async () => {
    const invoke = fakeMemory({ kind: 'failed', message: 'it exited with code 1' });
    nav.go('notes');
    render(NotesView);

    expect(await screen.findByText('Harness Memory did not answer. it exited with code 1')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_list').length).toBeGreaterThan(1));
  });
});
