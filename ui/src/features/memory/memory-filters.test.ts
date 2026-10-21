import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { ALL, calls, EMPTY_PROJECT, fakeMemory, filterChoices, NO_NEWER_RELEASE, noteRow, pickFilter, registerMemoryHooks, REPO_REACH, rowButton, setProjectScopes, SCOPES, showMemory, showNotes } from './memory-test-support';
import NotesView from './NotesView.svelte';

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

describe('the Scope filter', () => {
  const WIDGET = 'git:github.com/acme/widget';

  /** The sample notes with the first repository ones moved to another repository. */
  function inTwoRepositories() {
    let moved = 0;

    return sampleNotes().map((note) => (note.scope === 'repo' && moved++ < 5 ? { ...note, repo: 'github.com/acme/widget' } : note));
  }

  it('lists every repository by name with its count, and keeps the ones what is typed matches', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, inTwoRepositories());
    await showNotes();

    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 70', 'demo useful/harness-desktop 50', 'legacy-api 0', 'research 0', 'acme/widget Remote only 5', 'Global 15']);

    await fireEvent.click(screen.getByRole('button', { name: 'Scope: All' }));
    const menu = await screen.findByRole('menu', { name: 'Scope' });
    await fireEvent.input(within(menu).getByRole('searchbox', { name: 'Search repositories' }), { target: { value: 'WIDG' } });

    expect(within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['acme/widget Remote only 5']);

    await fireEvent.input(within(menu).getByRole('searchbox', { name: 'Search repositories' }), { target: { value: 'nothing' } });

    expect(within(menu).queryAllByRole('menuitemradio')).toHaveLength(0);
    expect(within(menu).getByText('No repository matches')).toBeInTheDocument();
  });

  it('asks for one repository by its key with its own notes only, every repository with none, and the global ones with no repository', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, inTwoRepositories());
    await showNotes();
    const lastAsk = () => calls(invoke, 'memory_notes_list').at(-1);

    expect(lastAsk()).toMatchObject(ALL);
    expect(sent(lastAsk(), 'scope')).toBeUndefined();

    await pickFilter('Scope: All', /^acme\/widget/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ project: null, repos: { kind: 'repo', key: WIDGET }, scope: 'repo', offset: 0 }));
    await waitFor(() => expect(screen.getByText('5 notes')).toBeInTheDocument());

    await pickFilter('Scope: acme/widget', /^Global/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ project: null, scope: 'global' }));
    expect(sent(lastAsk(), 'repos')).toBeUndefined();
    await waitFor(() => expect(screen.getByText('15 notes')).toBeInTheDocument());
  });

  it('offers a project on this computer that has no notes, and keeps it chosen with nothing to show', async () => {
    const invoke = fakeMemory();
    await showNotes();
    const lastAsk = () => calls(invoke, 'memory_notes_list').at(-1);

    await pickFilter('Scope: All', /^research/);

    await waitFor(() => expect(lastAsk()).toMatchObject({ project: null, repos: { kind: 'repo', key: `path:${EMPTY_PROJECT}` }, scope: 'repo' }));
    expect(await screen.findByText('No notes match these filters.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Scope: research' })).toBeInTheDocument();
    expect(viewState.memoryReach).toBe(`repo:path:${EMPTY_PROJECT}`);
    expect(await filterChoices('Scope: research', 'Scope')).toContain('research 0');
  });

  it('lists a repository only the remote database has as Remote only', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, inTwoRepositories());
    await showNotes();

    expect(await filterChoices('Scope: All', 'Scope')).toContain('acme/widget Remote only 5');
  });

  it('lets go of a repository only the remote database had once it has no notes', async () => {
    viewState.memoryReach = `repo:${WIDGET}`;
    fakeMemory();
    nav.go('notes');
    render(NotesView);

    await waitFor(() => expect(viewState.memoryReach).toBe('all'));
    await screen.findByRole('grid', { name: 'Notes' });
    expect(screen.getByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
  });

  it('shows a project without a repository key but does not let it be chosen', async () => {
    fakeMemory();
    await showNotes();

    await fireEvent.click(screen.getByRole('button', { name: 'Scope: All' }));
    const menu = await screen.findByRole('menu', { name: 'Scope' });

    expect(within(menu).getByRole('menuitemradio', { name: /^legacy-api/ })).toBeDisabled();
    expect(within(menu).getByRole('menuitemradio', { name: /^demo/ })).toBeEnabled();
  });

  it('keeps a project listed with the key Harness Memory gives it once that is known', async () => {
    setProjectScopes(SCOPES.map((scope) => (scope.name === 'legacy-api' ? { ...scope, key: 'git:github.com/acme/legacy-api' } : scope)));
    fakeMemory();
    await showNotes();

    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 70', 'demo useful/harness-desktop 55', 'legacy-api acme/legacy-api 0', 'research 0', 'Global 15']);
  });

  it('reads a note of another repository by the same choice, so it opens whichever chat is in front', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, inTwoRepositories());
    await showNotes();

    await fireEvent.click(noteRow(3) as HTMLElement);

    await waitFor(() => expect(calls(invoke, 'memory_note_get').at(-1)).toMatchObject({ id: 3, ...ALL }));
  });
});

describe('the filters of the notes', () => {
  it('are Scope and then Kind, both on All, with the count of notes in the row once', async () => {
    fakeMemory();
    await showNotes();

    expect(inOrder('Scope: All', 'Kind: All')).toBe(true);
    expect(screen.getAllByText('70 notes')).toHaveLength(1);
    expect(screen.queryByRole('radiogroup')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Clear the Scope filter' })).toBeNull();
    expect(screen.queryByText('Update Harness Memory to use the Scope filter together with the others.')).toBeNull();
  });

  it('count their choices, the scopes under the kind and the kinds under the scope', async () => {
    fakeMemory();
    await showNotes();

    // The repositories are counted whatever kind is chosen: they are the ones that have notes.
    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 70', 'demo useful/harness-desktop 55', 'legacy-api 0', 'research 0', 'Global 15']);
    expect(await filterChoices('Kind: All', 'Kind')).toEqual(ALL_KINDS);

    await pickFilter('Scope: All', /^Global/);

    await waitFor(() => expect(screen.getByText('15 notes')).toBeInTheDocument());
    expect(await filterChoices('Kind: All', 'Kind')).toEqual(GLOBAL_KINDS);

    await pickFilter('Scope: Global', /^All/);
    await pickFilter('Kind: All', /^Decision/);

    await waitFor(() => expect(screen.getByText('11 notes')).toBeInTheDocument());
  });

  it('ask the server for the scope alone, the kind alone, and both together, from the first page', async () => {
    const invoke = fakeMemory();
    viewState.notesPage = 1;
    await showNotes();
    const lastAsk = () => calls(invoke, 'memory_notes_list').at(-1);

    await pickFilter('Scope: All', /^demo/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', offset: 0 }));
    expect(sent(lastAsk(), 'kind')).toBeUndefined();
    expect(viewState.memoryReach).toBe(REPO_REACH);
    await waitFor(() => expect(screen.getByText('55 notes')).toBeInTheDocument());

    await pickFilter('Kind: All', /^Decision/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', kind: 'decision', offset: 0 }));
    await waitFor(() => expect(screen.getByText('8 notes')).toBeInTheDocument());
    // Note 3 is a decision of the repository, note 4 a fix.
    expect(noteRow(3)).not.toBeNull();
    expect(noteRow(4)).toBeNull();

    await pickFilter('Scope: demo', /^Global/);
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
    expect(viewState.memoryReach).toBe('global');

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
    expect(screen.queryByText('Update Harness Memory to use the Scope filter together with the others.')).toBeNull();
  });

  it('count their choices, the scopes under the status and the status under the scope', async () => {
    fakeMemory();
    await showMemory();

    expect(await filterChoices('Scope: All', 'Scope')).toEqual(['All 5', 'demo useful/harness-desktop 4', 'legacy-api 0', 'research 0', 'Global 1']);
    expect(await filterChoices('Status: All', 'Status')).toEqual(['All 5', 'Needs review 2', 'Trusted 3']);

    await pickFilter('Scope: All', /^Global/);
    await waitFor(() => expect(screen.getByText('1 script')).toBeInTheDocument());
    expect(await filterChoices('Status: All', 'Status')).toEqual(['All 1', 'Needs review 1', 'Trusted 0']);

    await pickFilter('Scope: Global', /^All/);
    await pickFilter('Status: All', /^Trusted/);
    await waitFor(() => expect(screen.getByText('3 scripts')).toBeInTheDocument());
  });

  it('ask the server for scope and review, never for a group next to them', async () => {
    const invoke = fakeMemory();
    viewState.scriptsPage = 1;
    await showMemory();
    const lastAsk = () => calls(invoke, 'memory_scripts_list').at(-1);

    await pickFilter('Scope: All', /^demo/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', offset: 0 }));

    await pickFilter('Status: All', /^Trusted/);
    await waitFor(() => expect(lastAsk()).toMatchObject({ scope: 'repo', review: 'trusted' }));
    await waitFor(() => expect(rowButton('disk-hogs')).toBeNull());
    expect(rowButton('open-prs')).not.toBeNull();
    expect(rowButton('lint-changed')).not.toBeNull();
    expect(rowButton('ci-status')).not.toBeNull();
    expect(rowButton('deploy-check')).toBeNull();

    await pickFilter('Scope: demo', /^Global/);
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
    expect(viewState.memoryReach).toBe('global');

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Scope filter' }));

    await waitFor(() => expect(rowButton('open-prs')).not.toBeNull());
  });
});

describe('an older Harness Memory', () => {
  const HINT = 'Update Harness Memory to use the Scope filter together with the others.';

  it('gets the Kind filter next to the Scope filter on the notes, never asked to combine them, and a hint to update', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, undefined, false);
    await showNotes();

    expect(await screen.findByRole('button', { name: 'Kind: All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
    expect(await screen.findByText(HINT)).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Update Harness Memory' })).toBeInTheDocument();

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
    expect(screen.queryByRole('button', { name: /^Status: / })).toBeNull();
    expect(await screen.findByText(HINT)).toBeInTheDocument();
    expect(await filterChoices('Show: All', 'Show')).toEqual(['All 5', 'Needs review 2', 'This repository 3', 'Global 0']);

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

  it('counts the groups of the Show filter apart: a script that needs review is in no scope group', async () => {
    const [repoNeedsReview, globalNeedsReview] = [sampleScripts().find((script) => script.name === 'deploy-check')!, sampleScripts().find((script) => script.name === 'disk-hogs')!];
    const globalTrusted = { ...globalNeedsReview, trusted: true, approval: 'approved' as const };
    fakeMemory({ kind: 'available' }, [repoNeedsReview, globalTrusted], undefined, undefined, false);
    await showMemory();

    expect(await filterChoices('Show: All', 'Show')).toEqual(['All 2', 'Needs review 1', 'This repository 0', 'Global 1']);
  });

  it('shows no hint and no update button when no newer Harness Memory release is known', async () => {
    fakeMemory({ kind: 'available' }, undefined, NO_NEWER_RELEASE, undefined, false);
    await showMemory();

    expect(await screen.findByRole('button', { name: 'Show: All' })).toBeInTheDocument();
    await waitFor(() => expect(ecosystem.state).toBe('ready'));
    await waitFor(() => expect(ecosystem.releasesPending).toBe(false));
    expect(screen.queryByText(HINT)).toBeNull();
    expect(screen.queryByRole('button', { name: 'Update Harness Memory' })).toBeNull();
  });

  it('shows no hint and no update button when the release lookup failed', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, undefined, false);
    const rest = invoke.getMockImplementation()!;
    invoke.mockImplementation(async (name, args) => {
      if (name === 'ecosystem_releases') {
        throw new IpcFailure({ code: 'internal', message: 'offline' });
      }

      return rest(name, args);
    });
    await showMemory();

    expect(await screen.findByRole('button', { name: 'Show: All' })).toBeInTheDocument();
    await waitFor(() => expect(ecosystem.state).toBe('ready'));
    await waitFor(() => expect(ecosystem.releasesPending).toBe(false));
    expect(screen.queryByText(HINT)).toBeNull();
    expect(screen.queryByRole('button', { name: 'Update Harness Memory' })).toBeNull();
  });

  it('adds the Status filter once Harness Memory is updated from the hint, after asking first', async () => {
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

    expect(await screen.findByRole('button', { name: 'Status: All' })).toBeInTheDocument();
    expect(screen.queryByText(HINT)).toBeNull();
    expect(calls(invoke, 'setup_install_memory')).toEqual([expect.objectContaining({ confirmed: true })]);
  });
});
