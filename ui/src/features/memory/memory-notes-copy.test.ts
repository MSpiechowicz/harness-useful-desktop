import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { NoteContent } from '$lib/ipc/bindings/NoteContent';
import type { NotesCopyResult } from '$lib/ipc/bindings/NotesCopyResult';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls, fakeMemory, registerMemoryHooks, showNotes, withTeamDatabase } from './memory-test-support';
import { notesTeam } from './notes-team.svelte';
import NotesCopyDialog from './NotesCopyDialog.svelte';

registerMemoryHooks();

const onThisComputer = (notes: NoteContent[]) => notes.filter((note) => note.place === 'local');

/** More notes on this computer than one page of conflicts holds. */
const manyLocal = (): NoteContent[] => Array.from({ length: 300 }, (_, index) => ({ ...sampleNotes()[3], id: 5000 + index, title: `Local note ${index}`, place: 'local' as const }));

/** The notes plus a remote note hidden on this computer that has the title of the third note saved here, which a copy would take as a conflict: that one is blocked instead. */
function withBlockedNote() {
  const notes = sampleNotes();
  const blocked = onThisComputer(notes)[2];

  return { notes: [...notes, { ...blocked, id: 9100, place: 'team' as const, hidden: true }], blocked };
}

async function openDialog() {
  const onclose = vi.fn();
  render(NotesCopyDialog, { open: true, onclose });
  await screen.findByRole('dialog');

  return onclose;
}

/** The checkboxes of the conflicts table, by the title they name. */
const pickBox = (title: string) => screen.getByRole('checkbox', { name: `Overwrite the remote copy of ${title}` });

describe('the banner for notes saved on this computer', () => {
  it('says how many notes the scopes on the remote database leave stranded, across all projects', async () => {
    const invoke = fakeMemory();
    withTeamDatabase(invoke, { repositoryNotes: 'remote', globalNotes: 'remote' });
    await showNotes();

    const stranded = onThisComputer(sampleNotes()).length;

    expect(await screen.findByText(`${stranded} notes saved on this computer are not in the remote database.`)).toBeInTheDocument();
  });

  it('counts only the scopes that read the remote database alone', async () => {
    const invoke = fakeMemory();
    withTeamDatabase(invoke, { repositoryNotes: 'remote', globalNotes: 'remoteLocal' });
    await showNotes();

    const repo = onThisComputer(sampleNotes()).filter((note) => note.scope === 'repo').length;

    expect(await screen.findByText(`${repo} notes saved on this computer are not in the remote database.`)).toBeInTheDocument();
  });

  it.each([
    ['saving on this computer', 'local'],
    ['reading both', 'remoteLocal'],
  ] as const)('is not shown while the scopes are %s', async (_name, routing) => {
    const invoke = fakeMemory();
    withTeamDatabase(invoke, { repositoryNotes: routing, globalNotes: routing });
    await showNotes();

    await waitFor(() => expect(calls(invoke, 'memory_remote_get')).not.toHaveLength(0));
    expect(screen.queryByText(/saved on this computer are not in the remote database/)).toBeNull();
  });

  it('is dismissed for the count it was dismissed at, and returns when more notes are stranded', async () => {
    const notes = sampleNotes();
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    withTeamDatabase(invoke, { repositoryNotes: 'remote', globalNotes: 'remote' });
    await showNotes();

    await fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByText(/not in the remote database/)).toBeNull());

    // The same count on the next visit: still dismissed.
    cleanup();
    const again = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    withTeamDatabase(again, { repositoryNotes: 'remote', globalNotes: 'remote' });
    await showNotes();
    await waitFor(() => expect(calls(again, 'memory_remote_get')).not.toHaveLength(0));
    expect(screen.queryByText(/not in the remote database/)).toBeNull();

    // One more note on this computer: the banner comes back.
    cleanup();
    const grown = fakeMemory({ kind: 'available' }, undefined, undefined, [...notes, { ...notes[3], id: 9000, title: 'One more', place: 'local' as const }]);
    withTeamDatabase(grown, { repositoryNotes: 'remote', globalNotes: 'remote' });
    await showNotes();

    expect(await screen.findByText(`${onThisComputer(notes).length + 1} notes saved on this computer are not in the remote database.`)).toBeInTheDocument();
    expect(notesTeam.dismissedAt).toBe(onThisComputer(notes).length);
  });

  it('opens the copy dialog', async () => {
    const invoke = fakeMemory();
    withTeamDatabase(invoke, { repositoryNotes: 'remote', globalNotes: 'remote' });
    await showNotes();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy…' }));

    expect(await screen.findByText(`Copy ${onThisComputer(sampleNotes()).length} notes saved on this computer to the remote database?`)).toBeInTheDocument();
  });

  it('deletes what is on this computer after a confirmation', async () => {
    const invoke = fakeMemory();
    withTeamDatabase(invoke, { repositoryNotes: 'remote', globalNotes: 'remote' });
    await showNotes();

    await fireEvent.click(await screen.findByRole('button', { name: 'Delete from this computer…' }));

    const stranded = onThisComputer(sampleNotes()).length;
    expect(await screen.findByText(`Delete ${stranded} notes from this computer?`)).toBeInTheDocument();
    expect(screen.getByText(/Deletes every note saved on this computer in the scopes that read the remote database/)).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_discard_local')).toHaveLength(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Delete from this computer' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_discard_local')).toEqual([{ ids: 'all' }]));
    await waitFor(() => expect(screen.queryByText(/not in the remote database/)).toBeNull());
    expect(toasts.items.at(-1)).toMatchObject({ title: `${stranded} notes deleted from this computer`, tone: 'success' });

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });
});

describe('copying the notes to the remote database', () => {
  it('compares first, asks about all repositories and leaves every conflict unticked', async () => {
    const invoke = fakeMemory();
    const onclose = await openDialog();

    const local = onThisComputer(sampleNotes()).length;
    expect(await screen.findByText(`Copy ${local} notes saved on this computer to the remote database?`)).toBeInTheDocument();
    expect(screen.getByText(/This covers all projects/)).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([{ kind: 'notes' }]);

    const boxes = screen.getAllByRole('checkbox');
    expect(boxes.length).toBeGreaterThan(0);
    expect(boxes.every((box) => box.getAttribute('aria-checked') === 'false')).toBe(true);

    await fireEvent.click(screen.getByRole('button', { name: 'Not now' }));

    expect(onclose).toHaveBeenCalledOnce();
    expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(0);
  });

  it('overwrites nothing when nothing is ticked', async () => {
    const invoke = fakeMemory();
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ overwrite: 'none' });
  });

  it('sends only the ticked conflicts', async () => {
    const invoke = fakeMemory();
    await openDialog();
    await screen.findByRole('table', { name: 'Notes that differ from the remote copy' });

    const [first] = await screen.findAllByRole('checkbox');
    const title = (first.getAttribute('aria-label') ?? '').replace('Overwrite the remote copy of ', '');
    const id = onThisComputer(sampleNotes()).find((note) => note.title === title)?.id;

    await fireEvent.click(pickBox(title));
    expect(screen.getByText(/^1 of \d+ will overwrite the remote copy$/)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ kind: 'notes', overwrite: [id] });
  });

  it('ticks every conflict on every page at once', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyLocal());
    await openDialog();
    await screen.findByRole('table', { name: 'Notes that differ from the remote copy' });

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ overwrite: 'all' });
  });

  it('names every other conflict when one of all is left out', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyLocal());
    await openDialog();
    await screen.findByRole('table', { name: 'Notes that differ from the remote copy' });

    const [first] = screen.getAllByRole('checkbox');
    const title = (first.getAttribute('aria-label') ?? '').replace('Overwrite the remote copy of ', '');
    const left = manyLocal().find((note) => note.title === title)?.id;

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    await fireEvent.click(pickBox(title));
    await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    const { overwrite } = calls(invoke, 'memory_notes_copy_apply')[0] as { overwrite: number[] };
    expect(overwrite).toHaveLength(99);
    expect(overwrite).not.toContain(left);
  });

  it('pages the conflicts on the server', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyLocal());
    await openDialog();
    await screen.findByRole('table', { name: 'Notes that differ from the remote copy' });

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_rows').at(-1)).toMatchObject({ kind: 'notes', filter: 'conflicts', offset: 50, limit: 50 }));
  });

  it('says how many stayed on this computer, and deletes them after a confirmation', async () => {
    const invoke = fakeMemory();
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    const dialog = await screen.findByRole('dialog', { name: 'Copy finished' });
    const stays = within(dialog).getByText('Stay on this computer').nextElementSibling?.textContent;
    expect(Number(stays)).toBeGreaterThan(0);
    expect(toasts.items.at(-1)).toMatchObject({ tone: 'success', description: `${stays} notes stay on this computer.` });

    // An agent saves a note after the comparison: the copy did not leave it, so the dialog must not delete it.
    const leftover = ((await invoke.mock.results.find((_, index) => invoke.mock.calls[index][0] === 'memory_notes_copy_apply')?.value) as NotesCopyResult).leftover;
    expect(leftover).toHaveLength(Number(stays));
    const saved = await ipc.invoke('memory_note_save', { project: null, draft: { title: 'Saved after the preview', body: 'Later.', kind: 'note', tags: [], entities: [], scope: 'global' } });

    await fireEvent.click(within(dialog).getByRole('button', { name: `Delete ${stays} notes from this computer…` }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Delete from this computer' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_discard_local')).toEqual([{ ids: leftover }]));
    expect(leftover).not.toContain((saved as { id: number }).id);
    expect(toasts.items.at(-1)).toMatchObject({ title: `${stays} notes deleted from this computer` });
    await waitFor(() => expect(within(dialog).queryByRole('button', { name: /from this computer…/ })).toBeNull());
  });

  it('counts the copied notes when only a relation failed, and offers nothing to delete', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    invoke.mockImplementation((async (name: CommandName, args: never) => {
      const answer = await real(name as never, args);

      return name === 'memory_notes_copy_apply'
        ? {
            ...(answer as NotesCopyResult),
            counts: { added: 3, updated: 1, overwritten: 1, identical: 2, teamNewer: 0, conflicts: 0, failed: 0 },
            deleted: 0,
            kept: 4,
            leftover: [],
            failed: [{ localId: null, title: null, message: 'relation failed' }],
          }
        : answer;
    }) as typeof ipc.invoke);
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    const dialog = await screen.findByRole('dialog', { name: 'Copy finished' });
    const stat = (label: string) => within(dialog).getByText(label).nextElementSibling?.textContent;
    expect(stat('Copied or already there')).toBe('7');
    expect(stat('Stay on this computer')).toBe('4');
    expect(stat('Failed')).toBe('0');
    expect(within(dialog).getByText(/Some notes could not be copied/)).toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: /from this computer…/ })).toBeNull();
    expect(toasts.items.at(-1)).toMatchObject({ title: '7 notes copied', tone: 'warning' });
  });

  it('counts a failed note once however many of its relations failed with it', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    invoke.mockImplementation((async (name: CommandName, args: never) => {
      const answer = await real(name as never, args);

      return name === 'memory_notes_copy_apply'
        ? {
            ...(answer as NotesCopyResult),
            counts: { added: 3, updated: 1, overwritten: 1, identical: 2, teamNewer: 0, conflicts: 0, failed: 1 },
            deleted: 0,
            kept: 4,
            leftover: [],
            failed: [
              { localId: 7, title: 'Note', message: 'note failed' },
              { localId: 7, title: 'Note', message: 'relation failed' },
              { localId: 7, title: 'Note', message: 'relation failed' },
              { localId: null, title: null, message: 'relation failed' },
            ],
          }
        : answer;
    }) as typeof ipc.invoke);
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    const dialog = await screen.findByRole('dialog', { name: 'Copy finished' });
    expect(within(dialog).getByText('Failed').nextElementSibling?.textContent).toBe('1');
  });

  it('says why the comparison failed and tries again', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    let failing = true;
    invoke.mockImplementation(((name: CommandName, args: never) => {
      if (name === 'memory_notes_copy_preview' && failing) {
        return Promise.reject(IpcFailure.from({ code: 'process', message: 'the remote database did not answer' }));
      }

      return real(name as never, args);
    }) as typeof ipc.invoke);
    await openDialog();

    expect(await screen.findByRole('alert')).toHaveTextContent('the remote database did not answer');

    failing = false;
    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    expect(await screen.findByRole('button', { name: 'Copy' })).toBeInTheDocument();
  });
});

describe('notes the remote database blocks', () => {
  it('lists a note whose title is hidden on this computer with the reason, never tickable and not counted as copyable', async () => {
    const { notes, blocked } = withBlockedNote();
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await openDialog();

    expect(await screen.findByText(`Copy ${onThisComputer(notes).length - 1} notes saved on this computer to the remote database?`)).toBeInTheDocument();
    expect(screen.getByText('Blocked').nextElementSibling?.textContent).toBe('1');

    const table = await screen.findByRole('table', { name: 'Notes that cannot be copied' });
    expect(within(table).getByText(blocked.title)).toBeInTheDocument();
    expect(within(table).getByText(/unhide it to copy this one/)).toBeInTheDocument();
    expect(within(table).queryAllByRole('checkbox')).toHaveLength(0);
    expect(screen.queryByRole('checkbox', { name: `Overwrite the remote copy of ${blocked.title}` })).toBeNull();
    expect(calls(invoke, 'memory_notes_copy_rows')).toContainEqual(expect.objectContaining({ kind: 'notes', filter: 'failed', offset: 0 }));
  });

  it('never sends a blocked note, even when every conflict is ticked', async () => {
    const { notes, blocked } = withBlockedNote();
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await openDialog();
    await screen.findByRole('table', { name: 'Notes that differ from the remote copy' });

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    const [first] = screen.getAllByRole('checkbox');
    await fireEvent.click(pickBox((first.getAttribute('aria-label') ?? '').replace('Overwrite the remote copy of ', '')));
    await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    const { overwrite } = calls(invoke, 'memory_notes_copy_apply')[0] as { overwrite: number[] };
    expect(overwrite).not.toContain(blocked.id);
  });

  it('keeps a blocked note out of what was copied and out of what is offered for deletion', async () => {
    const { notes, blocked } = withBlockedNote();
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, notes);
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    const dialog = await screen.findByRole('dialog', { name: 'Copy finished' });
    const result = (await invoke.mock.results.find((_, index) => invoke.mock.calls[index][0] === 'memory_notes_copy_apply')?.value) as NotesCopyResult;
    const stat = (label: string) => within(dialog).getByText(label).nextElementSibling?.textContent;

    expect(result.failed).toEqual([expect.objectContaining({ localId: blocked.id, title: blocked.title })]);
    expect(result.leftover).not.toContain(blocked.id);
    expect(stat('Copied or already there')).toBe(String(result.counts.added + result.counts.identical));
    expect(stat('Failed')).toBe('1');
    expect(stat('Stay on this computer')).toBe(String(result.kept));
    expect(within(dialog).getByRole('button', { name: `Delete ${result.leftover.length} notes from this computer…` })).toBeInTheDocument();
    expect(toasts.items.at(-1)).toMatchObject({ tone: 'warning' });
  });

  it('has nothing to copy when every note is blocked', async () => {
    const { notes } = withBlockedNote();
    const onlyBlocked = notes.filter((note) => note.place === 'team' || note.id === onThisComputer(notes)[2].id);
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, onlyBlocked);
    await openDialog();

    expect(await screen.findByRole('dialog', { name: '1 note cannot be copied' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Copy' })).toBeNull();
    expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(0);
  });
});
