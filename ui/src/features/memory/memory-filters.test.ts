import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, fakeMemory, filterChoices, noteRow, pickFilter, registerMemoryHooks, rowButton, showMemory, showNotes } from './memory-test-support';

registerMemoryHooks();

const ALL_KINDS = ['All 70', 'Decision 11', 'Fix 10', 'Problem 10', 'Outcome 10', 'Optimisation 10', 'Location 10', 'Note 9'];
const GLOBAL_KINDS = ['All 15', 'Decision 3', 'Fix 2', 'Problem 2', 'Outcome 1', 'Optimisation 2', 'Location 2', 'Note 3'];

/** What a request to the server carried under one name; a filter not asked for is not there. */
function sent(asked: unknown, name: string): unknown {
  return (asked as Record<string, unknown>)[name];
}

/** The filters sit in this order in the row above the table. */
function inOrder(first: string, second: string): boolean {
  const before = screen.getByRole('button', { name: first });
  const after = screen.getByRole('button', { name: second });

  return Boolean(before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING);
}

describe('the filters of the notes', () => {
  it('are Scope and then Kind, both on All, with the count of notes in the row once', async () => {
    fakeMemory();
    await showNotes();

    expect(inOrder('Scope: All', 'Kind: All')).toBe(true);
    expect(screen.getAllByText('70 notes')).toHaveLength(1);
    expect(screen.queryByRole('radiogroup')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Clear the Scope filter' })).toBeNull();
    expect(screen.queryByText('Update Harness Memory to add the Scope filter.')).toBeNull();
  });

  it('count their choices, the scopes under the kind and the kinds under the scope', async () => {
    fakeMemory();
    await showNotes();

    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 70', 'This repository 55', 'Global 15']);
    expect(await filterChoices('Kind: All', 'Kind')).toEqual(ALL_KINDS);

    await pickFilter('Scope: All', /^Global/);

    await waitFor(() => expect(screen.getByText('15 notes')).toBeInTheDocument());
    expect(await filterChoices('Kind: All', 'Kind')).toEqual(GLOBAL_KINDS);

    await pickFilter('Scope: Global', /^All/);
    await pickFilter('Kind: All', /^Decision/);

    await waitFor(() => expect(screen.getByText('11 notes')).toBeInTheDocument());
    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 11', 'This repository 8', 'Global 3']);
  });

  it('ask the server for the scope alone, the kind alone, and both together, from the first page', async () => {
    const invoke = fakeMemory();
    viewState.notesPage = 1;
    await showNotes();
    const lastAsk = () => calls(invoke, 'memory_notes_list').at(-1);

    await pickFilter('Scope: All', /^This repository/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', offset: 0 }));
    expect(sent(lastAsk(), 'kind')).toBeUndefined();
    expect(viewState.notesScope).toBe('repo');
    await waitFor(() => expect(screen.getByText('55 notes')).toBeInTheDocument());

    await pickFilter('Kind: All', /^Decision/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', kind: 'decision', offset: 0 }));
    await waitFor(() => expect(screen.getByText('8 notes')).toBeInTheDocument());
    // Note 3 is a decision of the repository, note 4 a fix.
    expect(noteRow(3)).not.toBeNull();
    expect(noteRow(4)).toBeNull();

    await pickFilter('Scope: This repository', /^Global/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'global', kind: 'decision' }));
    await waitFor(() => expect(screen.getByText('3 notes')).toBeInTheDocument());
    expect(noteRow(3)).toBeNull();
  });

  it('clear one at a time with the x in the button, and name themselves when nothing is left', async () => {
    const invoke = fakeMemory();
    await showNotes();

    expect(screen.queryByRole('button', { name: 'Clear the Kind filter' })).toBeNull();

    await pickFilter('Scope: All', /^Global/);
    await pickFilter('Kind: All', /^Outcome/);
    await waitFor(() => expect(screen.getByText('1 note')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Kind filter' }));

    await waitFor(() => expect(screen.getByText('15 notes')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Scope: Global' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Kind: All' })).toBeInTheDocument();
    expect(viewState.notesKind).toBeNull();
    expect(viewState.notesScope).toBe('global');

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Scope filter' }));

    await waitFor(() => expect(screen.getByText('70 notes')).toBeInTheDocument());
    expect(sent(calls(invoke, 'memory_notes_list').at(-1), 'scope')).toBeUndefined();
    expect(screen.queryByRole('button', { name: 'Clear the Scope filter' })).toBeNull();
  });

  it('say so when the two leave nothing, and a search keeps its own message', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, sampleNotes().filter((note) => note.scope !== 'global' || note.kind !== 'outcome'));
    await showNotes();

    await pickFilter('Scope: All', /^Global/);
    await pickFilter('Kind: All', /^Outcome 0/);

    expect(await screen.findByText('No notes match these filters.')).toBeInTheDocument();
    expect(screen.queryByRole('grid')).toBeNull();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search notes' }), { target: { value: 'zzz-nothing' } });

    expect(await screen.findByText('No note matches that search.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Scope: Global' })).toBeInTheDocument();
  });

  it('keep the choice of the view when it is left and shown again', async () => {
    fakeMemory();
    await showNotes();
    await pickFilter('Scope: All', /^Global/);

    cleanup();
    await showNotes();

    expect(screen.getByRole('button', { name: 'Scope: Global' })).toBeInTheDocument();
  });
});

describe('the filters of the scripts', () => {
  it('are Scope and then Status, both on All, in the same place as on the notes', async () => {
    fakeMemory();
    await showMemory();

    expect(inOrder('Scope: All', 'Status: All')).toBe(true);
    expect(screen.getAllByText('5 scripts')).toHaveLength(1);
    expect(screen.queryByRole('radiogroup')).toBeNull();
    expect(screen.queryByText('Update Harness Memory to add the Scope filter.')).toBeNull();
  });

  it('count their choices, the scopes under the status and the status under the scope', async () => {
    fakeMemory();
    await showMemory();

    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 5', 'This repository 4', 'Global 1']);
    expect(await filterChoices('Status: All', 'Status')).toEqual(['All 5', 'Needs review 2', 'Trusted 3']);

    await pickFilter('Scope: All', /^Global/);
    await waitFor(() => expect(screen.getByText('1 script')).toBeInTheDocument());
    expect(await filterChoices('Status: All', 'Status')).toEqual(['All 1', 'Needs review 1', 'Trusted 0']);

    await pickFilter('Scope: Global', /^All/);
    await pickFilter('Status: All', /^Trusted/);
    await waitFor(() => expect(screen.getByText('3 scripts')).toBeInTheDocument());
    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 3', 'This repository 3', 'Global 0']);
  });

  it('ask the server for scope and review, never for a group next to them', async () => {
    const invoke = fakeMemory();
    viewState.scriptsPage = 1;
    await showMemory();
    const lastAsk = () => calls(invoke, 'memory_scripts_list').at(-1);

    await pickFilter('Scope: All', /^This repository/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', offset: 0 }));

    await pickFilter('Status: All', /^Trusted/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', review: 'trusted' }));
    await waitFor(() => expect(rowButton('disk-hogs')).toBeNull());
    expect(rowButton('open-prs')).not.toBeNull();
    expect(rowButton('lint-changed')).not.toBeNull();
    expect(rowButton('ci-status')).not.toBeNull();
    expect(rowButton('deploy-check')).toBeNull();

    await pickFilter('Scope: This repository', /^Global/);
    await pickFilter('Status: Trusted', /^Needs review/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'global', review: 'needed' }));
    await waitFor(() => expect(rowButton('open-prs')).toBeNull());
    expect(rowButton('disk-hogs')).not.toBeNull();

    for (const asked of calls(invoke, 'memory_scripts_list')) {
      expect(sent(asked, 'group')).toBeUndefined();
    }
  });

  it('clear one at a time, and name themselves when nothing is left', async () => {
    fakeMemory();
    await showMemory();

    await pickFilter('Scope: All', /^Global/);
    await pickFilter('Status: All', /^Trusted/);

    expect(await screen.findByText('No scripts match these filters.')).toBeInTheDocument();
    expect(screen.queryByRole('grid')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Status filter' }));

    await waitFor(() => expect(rowButton('disk-hogs')).not.toBeNull());
    expect(rowButton('open-prs')).toBeNull();
    expect(viewState.scriptsStatus).toBeNull();
    expect(viewState.scriptsScope).toBe('global');

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Scope filter' }));

    await waitFor(() => expect(rowButton('open-prs')).not.toBeNull());
  });
});

describe('an older Harness Memory', () => {
  const HINT = 'Update Harness Memory to add the Scope filter.';

  it('gets the Kind filter alone on the notes, never asked for a scope, and a hint to update', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, undefined, false);
    viewState.notesScope = 'repo';
    await showNotes();

    expect(await screen.findByRole('button', { name: 'Kind: All' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Scope: / })).toBeNull();
    expect(screen.getByText(HINT)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update Harness Memory' })).toBeInTheDocument();

    await waitFor(() => expect(sent(calls(invoke, 'memory_notes_list').at(-1), 'scope')).toBeUndefined());

    await pickFilter('Kind: All', /^Decision/);

    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ kind: 'decision' }));
    expect(sent(calls(invoke, 'memory_notes_list').at(-1), 'scope')).toBeUndefined();
    await waitFor(() => expect(screen.getByText('11 notes')).toBeInTheDocument());
  });

  it('gets one Show filter of the old groups on the scripts, mapped to the group of the server', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, undefined, false);
    await showMemory();

    expect(await screen.findByRole('button', { name: 'Show: All' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^(Scope|Status): / })).toBeNull();
    expect(screen.getByText(HINT)).toBeInTheDocument();
    expect(await filterChoices('Show: All', 'Show')).toEqual(['All 5', 'Needs review 2', 'This repository 4', 'Global 1']);

    await pickFilter('Show: All', /^This repository/);

    await waitFor(() => expect(calls(invoke, 'memory_scripts_list').at(-1)).toMatchObject({ group: 'repo', offset: 0 }));
    expect(sent(calls(invoke, 'memory_scripts_list').at(-1), 'scope')).toBeUndefined();
    expect(sent(calls(invoke, 'memory_scripts_list').at(-1), 'review')).toBeUndefined();
    await waitFor(() => expect(rowButton('disk-hogs')).toBeNull());
    expect(rowButton('open-prs')).not.toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Show filter' }));

    await waitFor(() => expect(rowButton('disk-hogs')).not.toBeNull());
    expect(viewState.scriptsGroup).toBeNull();
  });

  it('adds the Scope filter once Harness Memory is updated from the hint, after asking first', async () => {
    let updated = false;
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, undefined, () => updated);
    const rest = invoke.getMockImplementation()!;
    const installed: DashboardInstallResult = { ok: true, summary: 'Installed harness-memory 1.2.0.', exitCode: 0, timedOut: false, truncated: false, version: '1.2.0', command: 'install.sh' };

    invoke.mockImplementation(async (name, args) => {
      if (name !== 'setup_install_memory') {
        return rest(name, args);
      }

      updated = true;

      return installed as never;
    });
    await showMemory();

    await fireEvent.click(await screen.findByRole('button', { name: 'Update Harness Memory' }));

    const dialog = await screen.findByRole('dialog');
    expect(calls(invoke, 'setup_install_memory')).toEqual([]);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Update Harness Memory' }));

    expect(await screen.findByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Status: All' })).toBeInTheDocument();
    expect(screen.queryByText(HINT)).toBeNull();
    expect(calls(invoke, 'setup_install_memory')).toEqual([expect.objectContaining({ confirmed: true })]);
  });
});
