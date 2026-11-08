import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';
import type { NotesCopyPreview } from '$lib/ipc/bindings/NotesCopyPreview';
import type { NotesCopyResult } from '$lib/ipc/bindings/NotesCopyResult';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import type { NoteContent } from '$lib/ipc/bindings/NoteContent';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls, fakeMemory, registerMemoryHooks } from './memory-test-support';
import NotesCopyDialog from './NotesCopyDialog.svelte';

registerMemoryHooks();

/** The remote notes, many more than one page of the plan holds. */
const manyRemote = (): NoteContent[] => Array.from({ length: 300 }, (_, index) => ({ ...sampleNotes()[0], id: 6000 + index, title: `Remote note ${index}`, place: 'team' as const, hidden: false }));

/** More remote notes than two collecting reads of 200 hold: three in five of them are missing on this computer. */
const moreThanTwoPagesRemote = (): NoteContent[] => Array.from({ length: 450 }, (_, index) => ({ ...sampleNotes()[0], id: 6000 + index, title: `Remote note ${index}`, place: 'team' as const, hidden: false }));

/** More notes saved on this computer than two collecting reads of 200 hold: the mock makes four in six of them copyable. */
const manyLocal = (): NoteContent[] => Array.from({ length: 450 }, (_, index) => ({ ...sampleNotes()[0], id: 8000 + index, title: `Local note ${index}`, place: 'local' as const, hidden: false }));

async function openDialog(directions: readonly NotesCopyMode[] | undefined, kind: MemoryCopyKind = 'notes') {
  const onclose = vi.fn();
  render(NotesCopyDialog, { open: true, kind, directions, onclose });
  await screen.findByRole('dialog');

  return onclose;
}

/** Makes a command answer with an error, once the test has set up what it needs. */
function failing(invoke: ReturnType<typeof fakeMemory>, command: CommandName, code: 'forbidden' | 'unsupported'): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    if (name === command) {
      throw IpcFailure.from({ code, message: 'raw server message' });
    }

    return real(name as never, args);
  }) as typeof ipc.invoke);
}

/** What the commands that were called answered, in order. */
async function answersOf<T>(invoke: ReturnType<typeof fakeMemory>, command: string): Promise<T[]> {
  const answers = invoke.mock.calls.flatMap(([name], index) => (name === command ? [invoke.mock.results[index].value as Promise<T>] : []));

  return Promise.all(answers);
}

const PICK_TABLE = 'Remote notes missing on this computer';
const pickBoxes = () => within(screen.getByRole('table', { name: PICK_TABLE })).getAllByRole('checkbox');
const copyButton = () => screen.getByRole('button', { name: 'Copy' });

describe('copying remote notes to this computer', () => {
  it('starts with nothing ticked and does not copy until something is', async () => {
    const invoke = fakeMemory();
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });

    const [preview] = await answersOf<NotesCopyPreview>(invoke, 'memory_notes_copy_preview');

    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([{ kind: 'notes', mode: 'toLocal' }]);
    expect(pickBoxes().every((box) => box.getAttribute('aria-checked') === 'false')).toBe(true);
    expect(screen.getByText(`0 of ${preview.counts.added} will be copied`)).toBeInTheDocument();
    expect(copyButton()).toBeDisabled();

    await fireEvent.click(copyButton());
    expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(0);
  });

  it('copies the rows ticked by their remote ids, leaves the remote as it is and says what it did', async () => {
    const invoke = fakeMemory();
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });

    await fireEvent.click(pickBoxes()[0]);
    await fireEvent.click(pickBoxes()[1]);
    expect(copyButton()).toBeEnabled();

    await fireEvent.click(copyButton());

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    const [rows] = await answersOf<{ rows: { teamId: number }[] }>(invoke, 'memory_notes_copy_rows');
    const [sent] = calls(invoke, 'memory_notes_copy_apply') as Record<string, unknown>[];

    expect(sent).toMatchObject({ kind: 'notes', mode: 'toLocal', pick: [rows.rows[0].teamId, rows.rows[1].teamId] });
    expect(sent).not.toHaveProperty('overwrite');
    expect(await screen.findByText('Copy finished')).toBeInTheDocument();
    expect(screen.getByText('The remote notes stay where they are. Each copy is its own note saved on this computer.')).toBeInTheDocument();
    expect(screen.getByText('Copied to this computer').parentElement).toHaveTextContent('2');
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Notes copied', description: '2 notes copied.', tone: 'success' });
  });

  it('takes every pickable row on every page with Tick all', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyRemote());
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    await fireEvent.click(copyButton());
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));

    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ mode: 'toLocal', pick: 'all' });
  });

  it('sends the remaining ids when one of all is unticked, collected from every page', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyRemote());
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });

    const [preview] = await answersOf<NotesCopyPreview>(invoke, 'memory_notes_copy_preview');
    const left = pickBoxes()[0].getAttribute('aria-label')?.replace(/^Copy | to this computer$/g, '') ?? '';

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    await fireEvent.click(screen.getByRole('checkbox', { name: `Copy ${left} to this computer` }));
    await fireEvent.click(copyButton());
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));

    const { pick } = calls(invoke, 'memory_notes_copy_apply')[0] as { pick: number[] };

    expect(pick).toHaveLength(preview.counts.added - 1);
    expect(new Set(pick).size).toBe(pick.length);
  });

  it('sends the remaining ids when one of all is unticked, with more entries than one collecting read holds', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, moreThanTwoPagesRemote());
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });

    const [preview] = await answersOf<NotesCopyPreview>(invoke, 'memory_notes_copy_preview');
    expect(preview.counts.added).toBeGreaterThan(200);

    const left = pickBoxes()[0].getAttribute('aria-label')?.replace(/^Copy | to this computer$/g, '') ?? '';

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    await fireEvent.click(screen.getByRole('checkbox', { name: `Copy ${left} to this computer` }));
    await fireEvent.click(copyButton());
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));

    const { pick } = calls(invoke, 'memory_notes_copy_apply')[0] as { pick: number[] };

    expect(pick).toHaveLength(preview.counts.added - 1);
    expect(new Set(pick).size).toBe(pick.length);
  });

  it('pages what can be picked on the server', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyRemote());
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });

    expect(pickBoxes()).toHaveLength(50);

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_rows')).toContainEqual(expect.objectContaining({ mode: 'toLocal', filter: 'pickable', offset: 50, limit: 50 })));
  });

  it('shows every remote note on request, with the reason a row cannot be picked and no tick that works', async () => {
    const invoke = fakeMemory();
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Show' })).getByRole('radio', { name: 'All' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_rows')).toContainEqual(expect.objectContaining({ mode: 'toLocal', filter: 'all' })));

    const table = screen.getByRole('table', { name: PICK_TABLE });
    const rows = within(table).getAllByRole('row');
    const same = rows.find((row) => within(row).queryByText('Already here, the same'));
    const differs = rows.find((row) => within(row).queryByText('A different version is here and stays as it is'));

    for (const row of [same, differs]) {
      const box = within(row!).getByRole('checkbox');

      expect(box).toBeDisabled();
      await fireEvent.click(box);
      expect(box).toHaveAttribute('aria-checked', 'false');
    }

    expect(screen.getByText(/^0 of \d+ will be copied$/)).toBeInTheDocument();
    expect(copyButton()).toBeDisabled();
  });

  it('says the copy is turned off in Settings when the comparison is refused', async () => {
    const invoke = fakeMemory();
    failing(invoke, 'memory_notes_copy_preview', 'forbidden');
    await openDialog(['toLocal']);

    expect(await screen.findAllByText('Copying to this computer is turned off. Turn on “Copy to this computer” in Settings, under Memory.')).not.toHaveLength(0);
    expect(screen.queryByRole('checkbox')).toBeNull();
    expect(screen.queryByText('raw server message')).toBeNull();
  });

  it('says so when the copy is refused after the user picked', async () => {
    const invoke = fakeMemory();
    await openDialog(['toLocal']);
    await screen.findByRole('table', { name: PICK_TABLE });
    await fireEvent.click(pickBoxes()[0]);

    failing(invoke, 'memory_notes_copy_apply', 'forbidden');
    await fireEvent.click(copyButton());

    expect(await screen.findAllByText('Copying to this computer is turned off. Turn on “Copy to this computer” in Settings, under Memory.')).not.toHaveLength(0);
    expect(screen.getByText('Could not compare with the remote database')).toBeInTheDocument();
  });

  it('says the installed Harness Memory cannot copy this way when the server lacks it', async () => {
    const invoke = fakeMemory();
    failing(invoke, 'memory_notes_copy_preview', 'unsupported');
    await openDialog(['toRemote']);

    expect(await screen.findAllByText('The installed Harness Memory cannot copy this way yet. Update it and try again.')).not.toHaveLength(0);
  });

  it('tells that copied scripts need approval before they run', async () => {
    fakeMemory();
    await openDialog(['toLocal'], 'scripts');

    expect(await screen.findByText(/A copied script needs your approval before it runs\./)).toBeInTheDocument();
    expect(screen.getByText('Copy remote scripts to this computer?')).toBeInTheDocument();
  });
});

const REMOTE_TABLE = 'Notes saved on this computer';
const remoteBoxes = () => within(screen.getByRole('table', { name: REMOTE_TABLE })).getAllByRole('checkbox');
const REPLACES = 'Differs from the remote copy. Ticking replaces the remote version.';
const titleOf = (box: HTMLElement) => (box.getAttribute('aria-label') ?? '').replace(/^Copy | to the remote$/g, '');
const rowOf = (text: string) => within(screen.getByRole('table', { name: REMOTE_TABLE })).getAllByRole('row').find((row) => within(row).queryByText(text));

describe('copying to the remote from the Sync dialog', () => {
  it('starts with nothing ticked and does not copy until something is', async () => {
    const invoke = fakeMemory();
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    const [preview] = await answersOf<NotesCopyPreview>(invoke, 'memory_notes_copy_preview');
    const pickable = preview.counts.added + preview.counts.updated + preview.counts.conflicts;

    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([{ kind: 'notes', mode: 'toRemote' }]);
    expect(remoteBoxes().every((box) => box.getAttribute('aria-checked') === 'false')).toBe(true);
    expect(screen.getByText(`0 of ${pickable} will be copied`)).toBeInTheDocument();
    expect(copyButton()).toBeDisabled();

    await fireEvent.click(copyButton());
    expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(0);

    await fireEvent.click(remoteBoxes()[0]);
    expect(copyButton()).toBeEnabled();
  });

  it('sends the ticked rows by their local ids and no overwrite, and says what it did', async () => {
    const invoke = fakeMemory();
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    const titles = [titleOf(remoteBoxes()[0]), titleOf(remoteBoxes()[1])];
    const ids = titles.map((title) => sampleNotes().find((note) => note.place === 'local' && note.title === title)?.id);

    await fireEvent.click(remoteBoxes()[0]);
    await fireEvent.click(remoteBoxes()[1]);
    await fireEvent.click(copyButton());

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    const [sent] = calls(invoke, 'memory_notes_copy_apply') as Record<string, unknown>[];

    expect(ids.every((id) => typeof id === 'number')).toBe(true);
    expect(sent).toMatchObject({ kind: 'notes', mode: 'toRemote', pick: ids });
    expect(sent).not.toHaveProperty('overwrite');
    expect(await screen.findByText('Copied notes are removed from this computer. The others stay here.')).toBeInTheDocument();
  });

  it('takes every pickable row on every page with Tick all', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyLocal());
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    expect(remoteBoxes()).toHaveLength(50);

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    await fireEvent.click(copyButton());
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));

    const [sent] = calls(invoke, 'memory_notes_copy_apply') as Record<string, unknown>[];

    expect(sent).toMatchObject({ mode: 'toRemote', pick: 'all' });
    expect(sent).not.toHaveProperty('overwrite');
  });

  it('sends the remaining ids when one of all is unticked, with more entries than one collecting read holds', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyLocal());
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    const [preview] = await answersOf<NotesCopyPreview>(invoke, 'memory_notes_copy_preview');
    const pickable = preview.counts.added + preview.counts.updated + preview.counts.conflicts;
    expect(pickable).toBeGreaterThan(200);

    const left = titleOf(remoteBoxes()[0]);

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    await fireEvent.click(screen.getByRole('checkbox', { name: `Copy ${left} to the remote` }));
    await fireEvent.click(copyButton());
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));

    const { pick } = calls(invoke, 'memory_notes_copy_apply')[0] as { pick: number[] };
    const unticked = manyLocal().find((note) => note.title === left)?.id;

    expect(pick).toHaveLength(pickable - 1);
    expect(new Set(pick).size).toBe(pick.length);
    expect(pick).not.toContain(unticked);
  });

  it('pages what can be picked on the server', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, manyLocal());
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_rows')).toContainEqual(expect.objectContaining({ mode: 'toRemote', filter: 'pickable', offset: 50, limit: 50 })));
  });

  it('lets a conflict be ticked, says that ticking replaces the remote version and sends it', async () => {
    const invoke = fakeMemory();
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    const conflict = rowOf(REPLACES)!;
    const box = within(conflict).getByRole('checkbox');
    const id = sampleNotes().find((note) => note.place === 'local' && note.title === titleOf(box))?.id;

    expect(box).toBeEnabled();

    await fireEvent.click(box);
    expect(box).toHaveAttribute('aria-checked', 'true');

    await fireEvent.click(copyButton());
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));

    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ mode: 'toRemote', pick: [id] });
  });

  it('shows every note on request, with the reason a row cannot be ticked and no tick that works', async () => {
    const invoke = fakeMemory();
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    expect(screen.queryByText('Already on the remote, the same')).toBeNull();

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Show' })).getByRole('radio', { name: 'All' }));
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_rows')).toContainEqual(expect.objectContaining({ mode: 'toRemote', filter: 'all' })));

    for (const reason of ['Already on the remote, the same', 'The remote version is newer and stays as it is']) {
      const box = within((await waitFor(() => rowOf(reason)!))).getByRole('checkbox');

      expect(box).toBeDisabled();
      await fireEvent.click(box);
      expect(box).toHaveAttribute('aria-checked', 'false');
    }

    expect(screen.getByText(/^0 of \d+ will be copied$/)).toBeInTheDocument();
    expect(copyButton()).toBeDisabled();
  });

  it('keeps Tick all and Untick all as ordinary buttons that say why one does nothing', async () => {
    fakeMemory();
    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    const untick = screen.getByRole('button', { name: 'Untick all' });

    expect(untick).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Tick all' })).toBeEnabled();

    await fireEvent.pointerEnter(untick.parentElement!);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('No row is ticked yet.');

    await fireEvent.click(screen.getByRole('button', { name: 'Tick all' }));
    expect(screen.getByRole('button', { name: 'Tick all' })).toBeDisabled();
    expect(untick).toBeEnabled();
  });

  it('offers no deletion of what stays on this computer, which a scope that reads both places cannot discard', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;

    // A server that reports something left on this computer: the way to delete it from here is stranded notes only.
    invoke.mockImplementation((async (name: CommandName, args: never) => {
      const answer = await real(name as never, args);

      return name === 'memory_notes_copy_apply' ? { ...(answer as NotesCopyResult), kept: 2, leftover: [1, 2] } : answer;
    }) as typeof ipc.invoke);

    await openDialog(['toRemote']);
    await screen.findByRole('table', { name: REMOTE_TABLE });

    await fireEvent.click(remoteBoxes()[0]);
    await fireEvent.click(copyButton());
    await screen.findByText('Copy finished');

    expect(screen.getByText('Stay on this computer').parentElement).toHaveTextContent('2');
    expect(screen.queryByRole('button', { name: /from this computer…/ })).toBeNull();
    expect(screen.getByRole('button', { name: 'Done' })).toBeInTheDocument();
  });

  it('still offers to delete what stays when the notes are stranded', async () => {
    fakeMemory();
    await openDialog(undefined);

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));
    await screen.findByText('Copy finished');

    expect(screen.getByRole('button', { name: /^Delete \d+ notes? from this computer…$/ })).toBeInTheDocument();
  });

  it('compares again in the direction the user switches to, with nothing ticked', async () => {
    const invoke = fakeMemory();
    await openDialog(['toRemote', 'toLocal']);
    await screen.findByRole('table', { name: REMOTE_TABLE });
    await fireEvent.click(remoteBoxes()[0]);

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Direction' })).getByRole('radio', { name: 'To this computer' }));

    await screen.findByRole('table', { name: PICK_TABLE });
    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([
      { kind: 'notes', mode: 'toRemote' },
      { kind: 'notes', mode: 'toLocal' },
    ]);
    expect(pickBoxes().every((box) => box.getAttribute('aria-checked') === 'false')).toBe(true);
    expect(copyButton()).toBeDisabled();
  });

  it('leaves the copy of stranded notes as it was: no direction, no mode', async () => {
    const invoke = fakeMemory();
    await openDialog(undefined);
    await screen.findByRole('table', { name: 'Notes that differ from the remote copy' });

    expect(screen.queryByRole('radiogroup', { name: 'Direction' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));
    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));

    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([{ kind: 'notes' }]);
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toEqual({ kind: 'notes', previewId: expect.any(String), overwrite: 'none' });
  });
});
