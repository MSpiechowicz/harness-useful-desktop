import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, fakeMemory, PROJECT, registerMemoryHooks, rowButton, showMemory, showNotes } from './memory-test-support';
import ScriptsView from './ScriptsView.svelte';

registerMemoryHooks();

async function chooseDeleteAll(label: string): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
  await fireEvent.click(await screen.findByRole('menuitem', { name: label }));
}

/** Opens the Scripts view where there is no table to show, only the words that say why, and waits for them. */
async function showWithoutTable(words: string): Promise<void> {
  nav.go('scripts');
  render(ScriptsView);
  await screen.findByText(words);
}

/** Makes the delete of everything fail the way a server that cannot write does. */
function failDeleteAll(invoke: ReturnType<typeof fakeMemory>): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;
  invoke.mockImplementation(((name: CommandName, args: never) =>
    name === 'memory_scripts_delete_all' || name === 'memory_notes_delete_all'
      ? Promise.reject(IpcFailure.from({ code: 'process', message: 'database is locked' }))
      : real(name as never, args)) as typeof ipc.invoke);
}

describe('deleting all scripts', () => {
  it('asks with the number and the team, deletes by project, closes the panel and reads the list again', async () => {
    const invoke = fakeMemory();
    await showMemory();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);
    await screen.findByRole('region', { name: 'open-prs' });

    await chooseDeleteAll('Delete all scripts…');

    expect(await screen.findByText('Delete all 5 scripts?')).toBeInTheDocument();
    expect(screen.getByText('4 of them are in the remote database, so your teammates lose them too.')).toBeInTheDocument();
    expect(calls(invoke, 'memory_scripts_delete_all')).toHaveLength(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Delete all scripts' }));

    expect(await screen.findByText('No scripts yet')).toBeInTheDocument();
    expect(calls(invoke, 'memory_scripts_delete_all')).toEqual([{ project: PROJECT }]);
    expect(viewState.scriptId).toBeNull();
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(toasts.items.at(-1)).toMatchObject({ title: '5 scripts deleted', description: '4 of them were in the remote database.', tone: 'success' });
  });

  it('leaves the read-only remote scripts out, says they stay and can be hidden, and tells how many stayed', async () => {
    const scripts = sampleScripts().map((script) => ({ ...script, readOnly: script.place === 'team' }));
    const stay = scripts.filter((script) => script.readOnly).length;
    const invoke = fakeMemory({ kind: 'available' }, scripts);
    await showMemory();

    await chooseDeleteAll('Delete all scripts…');

    expect(scripts.length - stay).toBe(1);
    expect(await screen.findByText('Delete 1 script?')).toBeInTheDocument();
    expect(screen.getByText(`${stay} remote scripts stay, you can hide them.`)).toBeInTheDocument();
    expect(screen.queryByText(/your teammates lose/)).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Delete all scripts' }));

    await waitFor(() => expect(calls(invoke, 'memory_scripts_delete_all')).toEqual([{ project: PROJECT }]));
    await waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ title: '1 script deleted', description: `${stay} remote scripts stay, you can hide them.` }));
  });

  it('says so instead of asking when only read-only remote scripts are left', async () => {
    const scripts = sampleScripts().map((script) => ({ ...script, place: 'team' as const, readOnly: true }));
    const invoke = fakeMemory({ kind: 'available' }, scripts);
    await showMemory();

    await chooseDeleteAll('Delete all scripts…');

    await waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ title: `Nothing to delete here. The ${scripts.length} remote scripts stay, you can hide them.`, tone: 'info' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(calls(invoke, 'memory_scripts_delete_all')).toHaveLength(0);
  });

  it('names the whole list even while a search narrows the table', async () => {
    fakeMemory();
    await showMemory();
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search scripts' }), { target: { value: 'pull requests' } });
    await screen.findByText('1 script');

    await chooseDeleteAll('Delete all scripts…');

    expect(await screen.findByText('Delete all 5 scripts?')).toBeInTheDocument();
  });

  it('keeps everything when it is cancelled', async () => {
    const invoke = fakeMemory();
    await showMemory();

    await chooseDeleteAll('Delete all scripts…');
    await fireEvent.click(await screen.findByRole('button', { name: 'Keep them' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(calls(invoke, 'memory_scripts_delete_all')).toHaveLength(0);
    expect(rowButton('open-prs')).not.toBeNull();
  });

  it('says why it failed inside the dialog, keeps the dialog and the rows, and can be tried again', async () => {
    const invoke = fakeMemory();
    failDeleteAll(invoke);
    await showMemory();

    await chooseDeleteAll('Delete all scripts…');
    await fireEvent.click(await screen.findByRole('button', { name: 'Delete all scripts' }));

    const dialog = await screen.findByRole('dialog');
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('Could not delete the scripts: database is locked');
    expect(within(dialog).getByRole('button', { name: 'Delete all scripts' })).toBeEnabled();
    expect(rowButton('open-prs')).not.toBeNull();
    expect(toasts.items).toHaveLength(0);
  });

  it('does not offer it when there is nothing to delete', async () => {
    fakeMemory({ kind: 'available' }, []);
    await showWithoutTable('No scripts yet');
    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

    expect(await screen.findByRole('menuitem', { name: 'Delete all scripts…' })).toBeDisabled();
  });

  it('does not offer it when Harness Memory is too old to answer', async () => {
    fakeMemory({ kind: 'outdated' });
    await showWithoutTable('Your memory needs an update');
    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

    expect(await screen.findByRole('menuitem', { name: 'Delete all scripts…' })).toBeDisabled();
  });
});

describe('deleting all notes', () => {
  it('asks for the notes, says how many are the team', async () => {
    const invoke = fakeMemory();
    const all = sampleNotes();
    const team = all.filter((note) => note.place === 'team').length;
    await showNotes();

    await chooseDeleteAll('Delete all notes…');

    expect(await screen.findByText(`Delete all ${all.length} notes?`)).toBeInTheDocument();
    expect(screen.getByText(`${team} of them are in the remote database, so your teammates lose them too.`)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Delete all notes' }));

    expect(await screen.findByText('No notes yet')).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_delete_all')).toEqual([{ project: PROJECT }]);
    expect(toasts.items.at(-1)).toMatchObject({ title: `${all.length} notes deleted` });
  });

  it('says only that some may be the team when the list is longer than one read shows', async () => {
    const local = Array.from({ length: 260 }, (_, index) => ({ ...sampleNotes()[3], id: 5000 + index, title: `local ${index}`, place: 'local' as const }));
    fakeMemory({ kind: 'available' }, undefined, undefined, local);
    await showNotes();

    await chooseDeleteAll('Delete all notes…');

    expect(await screen.findByText('Delete all 260 notes?')).toBeInTheDocument();
    expect(screen.getByText('Some of them may be in the remote database, so your teammates lose them too.')).toBeInTheDocument();
  });

  it('leaves the team out when every note is on this computer', async () => {
    const local = sampleNotes().map((note) => ({ ...note, place: 'local' as const }));
    fakeMemory({ kind: 'available' }, undefined, undefined, local);
    await showNotes();

    await chooseDeleteAll('Delete all notes…');

    expect(await screen.findByText(`Delete all ${local.length} notes?`)).toBeInTheDocument();
    expect(screen.queryByText(/remote database/)).toBeNull();
  });

  it('leaves the read-only remote notes out, says they stay and can be hidden, and tells how many stayed', async () => {
    const notes = sampleNotes().map((note) => ({ ...note, readOnly: note.place === 'team' }));
    const stay = notes.filter((note) => note.readOnly).length;
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await showNotes();

    await chooseDeleteAll('Delete all notes…');

    expect(await screen.findByText(`Delete all ${notes.length - stay} notes?`)).toBeInTheDocument();
    expect(screen.getByText(`${stay} remote notes stay, you can hide them.`)).toBeInTheDocument();
    expect(screen.queryByText(/your teammates lose/)).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Delete all notes' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_delete_all')).toEqual([{ project: PROJECT }]));
    await waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ title: `${notes.length - stay} notes deleted`, description: `${stay} remote notes stay, you can hide them.` }));
  });

  it('says so instead of asking when only read-only remote notes are left', async () => {
    const notes = sampleNotes().map((note) => ({ ...note, place: 'team' as const, readOnly: true }));
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await showNotes();

    await chooseDeleteAll('Delete all notes…');

    await waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ title: `Nothing to delete here. The ${notes.length} remote notes stay, you can hide them.`, tone: 'info' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(calls(invoke, 'memory_notes_delete_all')).toHaveLength(0);
  });
});
