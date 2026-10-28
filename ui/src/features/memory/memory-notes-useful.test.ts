import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import UsageMeter from '$lib/components/UsageMeter.svelte';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, fakeMemory, noteRow, pickFilter, registerMemoryHooks, REPO_REACH, showNotes } from './memory-test-support';

registerMemoryHooks();

/** The notes of a Harness Memory that counts nothing about how useful they were. */
const uncounted = () => sampleNotes().map((note) => ({ ...note, shown: null, useful: null, lastUsefulAt: null }));

/** Makes the server's feature line `notes-useful` listed or not. */
function withNotesUseful(invoke: ReturnType<typeof fakeMemory>, listed: boolean): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    if (name !== 'memory_remote_get') {
      return real(name as never, args);
    }

    return { ...(await real('memory_remote_get', undefined)), notesUsefulSupported: listed };
  }) as typeof ipc.invoke);
}

const cellOf = (id: number, text: RegExp | string) => within(noteRow(id) as HTMLElement).getByText(text);
const meterOf = (id: number) => within(noteRow(id) as HTMLElement).queryByRole('progressbar');
const fillOf = (meter: HTMLElement) => meter.firstElementChild as HTMLElement;

describe('how useful the notes were', () => {
  it('adds the columns of usefulness with the count, a meter and the last time, only where it is known', async () => {
    fakeMemory();
    await showNotes();

    expect(screen.getByRole('columnheader', { name: 'Useful in searches' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'Last useful' })).toBeInTheDocument();

    expect(cellOf(3, '11 of 14')).toBeInTheDocument();
    expect(meterOf(3)).toHaveAttribute('aria-valuenow', '11');
    expect(meterOf(3)).toHaveAttribute('aria-valuemax', '14');
    expect(meterOf(3)).toHaveAccessibleName('Found useful in searches');
    expect(cellOf(3, '3 hours ago')).toBeInTheDocument();
  });

  it('warns on the meter of a note found useful in under a fifth of its showings', async () => {
    fakeMemory();
    await showNotes();

    // 7 of 9 is high, 3 of 20 and 0 of 8 are low.
    expect(fillOf(meterOf(4) as HTMLElement)).toHaveClass('bg-accent');
    expect(fillOf(meterOf(6) as HTMLElement)).toHaveClass('bg-warning');
    expect(fillOf(meterOf(5) as HTMLElement)).toHaveClass('bg-warning');
  });

  it('says Never for a note shown but never useful, and shows no meter or time for one never shown', async () => {
    fakeMemory();
    await showNotes();

    expect(cellOf(5, '0 of 8')).toBeInTheDocument();
    expect(within(noteRow(5) as HTMLElement).getByText('Never')).toBeInTheDocument();

    expect(meterOf(8)).toBeNull();
    expect(within(noteRow(8) as HTMLElement).queryByText(/ of /)).toBeNull();
    expect(within(noteRow(8) as HTMLElement).queryByText('Never')).toBeNull();
    expect(within(noteRow(8) as HTMLElement).getAllByText('—')).toHaveLength(2);
  });

  it('shows a dash and no meter where the counts are unknown', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, uncounted());
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesUsefulSupported).toBe(true));

    expect(screen.queryByRole('progressbar')).toBeNull();
    expect(within(noteRow(3) as HTMLElement).getAllByText('—')).toHaveLength(2);
    expect(within(noteRow(3) as HTMLElement).queryByText('Never')).toBeNull();
  });

  it('sorts by either column on the server, the biggest and the newest first', async () => {
    const invoke = fakeMemory();
    await showNotes();
    const lastAsk = () => calls(invoke, 'memory_notes_list').at(-1);

    await fireEvent.click(screen.getByRole('button', { name: 'Useful in searches' }));
    await waitFor(() => expect(lastAsk()).toMatchObject({ sort: 'useful', order: 'desc', offset: 0 }));
    expect(screen.getByRole('columnheader', { name: 'Useful in searches' })).toHaveAttribute('aria-sort', 'descending');

    await fireEvent.click(screen.getByRole('button', { name: 'Last useful' }));
    await waitFor(() => expect(lastAsk()).toMatchObject({ sort: 'lastUseful', order: 'desc', offset: 0 }));

    await fireEvent.click(screen.getByRole('button', { name: 'Last useful' }));
    await waitFor(() => expect(lastAsk()).toMatchObject({ sort: 'lastUseful', order: 'asc' }));
  });

  it('narrows the list to the notes found useful, or to those shown often and never useful, asking the server', async () => {
    const invoke = fakeMemory();
    await showNotes();
    const lastAsk = () => calls(invoke, 'memory_notes_list').at(-1);

    expect(lastAsk()).toMatchObject({ useful: undefined });
    expect(screen.getByRole('button', { name: 'Usefulness: All' })).toBeInTheDocument();

    await pickFilter('Usefulness: All', /^Never useful/);

    await waitFor(() => expect(lastAsk()).toMatchObject({ useful: 'never', offset: 0 }));
    await waitFor(() => expect(noteRow(3)).toBeNull());
    expect(noteRow(5)).not.toBeNull();
    expect(noteRow(8)).toBeNull();

    await pickFilter('Usefulness: Never useful', /^Useful at least once/);

    await waitFor(() => expect(lastAsk()).toMatchObject({ useful: 'ever' }));
    await waitFor(() => expect(noteRow(5)).toBeNull());
    expect(noteRow(3)).not.toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Usefulness filter' }));

    await waitFor(() => expect(lastAsk()).toMatchObject({ useful: undefined }));
    await waitFor(() => expect(noteRow(8)).not.toBeNull());
  });

  it('explains what Never useful means', async () => {
    fakeMemory();
    await showNotes();

    await fireEvent.click(screen.getByRole('button', { name: 'Usefulness: All' }));

    expect(await screen.findByText('Shown 3+ times, never useful')).toBeInTheDocument();
  });

  it('keeps Delete all on while the filter leaves no note, as the list is narrowed', async () => {
    viewState.memoryReach = REPO_REACH;
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    // A server that counts the kinds under this filter too: nothing is left to count.
    invoke.mockImplementation((async (name: CommandName, args: never) => {
      const answer = await real(name as never, args);

      if (name === 'memory_notes_list' && (args as { useful?: string }).useful === 'ever') {
        return { ...(answer as object), total: 0, notes: [], kinds: [] };
      }

      return answer;
    }) as typeof ipc.invoke);
    await showNotes();

    await pickFilter('Usefulness: All', /^Useful at least once/);
    await screen.findByText('No notes match these filters.');

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

    expect(await screen.findByRole('menuitem', { name: 'Delete all notes…' })).toBeEnabled();
  });

  it('forgets the filter when the notes view is reset', () => {
    viewState.notesUseful = 'never';

    viewState.reset('notes');

    expect(viewState.notesUseful).toBeNull();
  });
});

describe('a server that does not count how useful notes were', () => {
  it('offers no columns and no filter, and no note carries a count', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, uncounted());
    withNotesUseful(invoke, false);
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesUsefulSupported).toBe(false));

    expect(noteRow(3)).not.toBeNull();
    expect(screen.queryByRole('columnheader', { name: 'Useful in searches' })).toBeNull();
    expect(screen.queryByRole('columnheader', { name: 'Last useful' })).toBeNull();
    expect(screen.queryByRole('button', { name: /^Usefulness/ })).toBeNull();
    expect(screen.queryByRole('progressbar')).toBeNull();
  });

  it('follows the feature line over what a row happens to carry', async () => {
    const invoke = fakeMemory();
    withNotesUseful(invoke, false);
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesUsefulSupported).toBe(false));

    expect(screen.queryByRole('columnheader', { name: 'Useful in searches' })).toBeNull();
    expect(screen.queryByRole('button', { name: /^Usefulness/ })).toBeNull();
  });

  it('drops a chosen filter and a sort by usefulness, and asks for the notes by time instead', async () => {
    const invoke = fakeMemory();
    withNotesUseful(invoke, false);
    viewState.notesUseful = 'never';
    viewState.notesSort = { key: 'useful', desc: true };
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesUsefulSupported).toBe(false));

    await waitFor(() => expect(viewState.notesUseful).toBeNull());
    await waitFor(() => expect(viewState.notesSort).toEqual({ key: 'updated', desc: true }));
    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ useful: undefined, sort: 'updated', order: 'desc' }));
    expect(screen.queryByRole('button', { name: /^Usefulness/ })).toBeNull();
    expect(screen.queryByText('Your memory needs an update')).toBeNull();
    expect(noteRow(3)).not.toBeNull();
    expect(noteRow(8)).not.toBeNull();
  });
});

describe('the meter', () => {
  const fill = (value: number, total: number) => {
    const { container } = render(UsageMeter, { value, total, label: 'Used' });

    return fillOf(container.querySelector('[role="progressbar"]') as HTMLElement);
  };

  it('is the accent from a fifth of the total up, and the warning tone below it', () => {
    expect(fill(1, 5)).toHaveClass('bg-accent');
    expect(fill(1, 6)).toHaveClass('bg-warning');
    expect(fill(0, 4)).toHaveClass('bg-warning');
  });

  it('fills the share of the total', () => {
    expect(fill(3, 4).style.width).toBe('75%');
  });
});
