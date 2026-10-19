import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { NotesCopyResult } from '$lib/ipc/bindings/NotesCopyResult';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls, fakeMemory, registerMemoryHooks, showMemory, withTeamDatabase } from './memory-test-support';
import { notesTeam, scriptsTeam } from './notes-team.svelte';
import NotesCopyDialog from './NotesCopyDialog.svelte';

registerMemoryHooks();

/** Six scripts saved on this computer, which a copy plans as two added, one already there, two that differ and one the remote has newer. */
const localScripts = (): ScriptDetail[] =>
  Array.from({ length: 6 }, (_, index) => ({
    ...sampleScripts()[1],
    id: 100 + index,
    name: `local-script-${index}`,
    scope: index % 2 === 0 ? ('repo' as const) : ('global' as const),
    repo: index % 2 === 0 ? 'github.com/useful/harness-desktop' : null,
  }));
const withLocal = (): ScriptDetail[] => [...sampleScripts().filter((script) => script.place === 'team'), ...localScripts()];

/** The scripts plus a remote script hidden on this computer with the name of the fourth one saved here, which a copy would add: that one is blocked instead. */
const withBlockedScript = (): ScriptDetail[] => [...withLocal(), { ...localScripts()[3], id: 900, place: 'team' as const, hidden: true }];

async function openDialog() {
  const onclose = vi.fn();
  render(NotesCopyDialog, { open: true, kind: 'scripts', onclose });
  await screen.findByRole('dialog');

  return onclose;
}

/** The checkbox of a conflict, by the script name it names. */
const pickBox = (name: string) => screen.getByRole('checkbox', { name: `Overwrite the remote copy of ${name}` });

describe('the banner for scripts saved on this computer', () => {
  it('counts the scripts of all repositories while scripts are on the remote database alone', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    withTeamDatabase(invoke, { repositoryNotes: 'local', globalNotes: 'local', scripts: 'remote' });
    await showMemory();

    expect(await screen.findByText('6 scripts saved on this computer are not in the remote database.')).toBeInTheDocument();
  });

  it.each([
    ['saving on this computer', 'local'],
    ['reading both', 'remoteLocal'],
  ] as const)('is not shown while scripts are %s', async (_name, scripts) => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    withTeamDatabase(invoke, { repositoryNotes: 'remote', globalNotes: 'remote', scripts });
    await showMemory();

    await waitFor(() => expect(calls(invoke, 'memory_remote_get')).not.toHaveLength(0));
    expect(screen.queryByText(/scripts? saved on this computer (is|are) not in the remote database/)).toBeNull();
  });

  it('is dismissed for the count it was dismissed at, apart from the notes banner, and returns when more scripts are stranded', async () => {
    const scripts = withLocal();
    const invoke = fakeMemory({ kind: 'available' }, scripts);
    withTeamDatabase(invoke, { repositoryNotes: 'local', globalNotes: 'local', scripts: 'remote' });
    await showMemory();

    await fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(screen.queryByText(/not in the remote database/)).toBeNull());

    expect(scriptsTeam.dismissedAt).toBe(6);
    expect(notesTeam.dismissedAt).toBe(0);
    expect(localStorage.getItem('harness.scriptsCopyDismissed')).toBe('6');
    expect(localStorage.getItem('harness.notesCopyDismissed')).not.toBe('6');

    // One more script on this computer: the banner comes back.
    cleanup();
    const grown = fakeMemory({ kind: 'available' }, [...scripts, { ...localScripts()[0], id: 900, name: 'one-more' }]);
    withTeamDatabase(grown, { repositoryNotes: 'local', globalNotes: 'local', scripts: 'remote' });
    await showMemory();

    expect(await screen.findByText('7 scripts saved on this computer are not in the remote database.')).toBeInTheDocument();
  });

  it('deletes everything on this computer after a confirmation that says so, without a scope', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    withTeamDatabase(invoke, { repositoryNotes: 'local', globalNotes: 'local', scripts: 'remote' });
    await showMemory();

    await fireEvent.click(await screen.findByRole('button', { name: 'Delete from this computer…' }));

    expect(await screen.findByText('Delete 6 scripts from this computer?')).toBeInTheDocument();
    expect(screen.getByText(/Deletes every script saved on this computer in the scopes that read the remote database/)).toBeInTheDocument();
    expect(calls(invoke, 'memory_scripts_discard_local')).toHaveLength(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Delete from this computer' }));

    await waitFor(() => expect(calls(invoke, 'memory_scripts_discard_local')).toEqual([{ ids: 'all' }]));
    expect(calls(invoke, 'memory_notes_discard_local')).toHaveLength(0);
    expect(toasts.items.at(-1)).toMatchObject({ title: '6 scripts deleted from this computer', tone: 'success' });
    await waitFor(() => expect(screen.queryByText(/not in the remote database/)).toBeNull());
  });

  it('opens the copy dialog for the scripts', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    withTeamDatabase(invoke, { repositoryNotes: 'local', globalNotes: 'local', scripts: 'remote' });
    await showMemory();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy…' }));

    expect(await screen.findByText('Copy 6 scripts saved on this computer to the remote database?')).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([{ kind: 'scripts' }]);
  });
});

describe('copying the scripts to the remote database', () => {
  it('compares the scripts, says so in its own words and leaves every conflict unticked', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    const onclose = await openDialog();

    expect(await screen.findByText('Copy 6 scripts saved on this computer to the remote database?')).toBeInTheDocument();
    expect(screen.getByText(/Scripts that differ from the remote copy stay unticked/)).toBeInTheDocument();
    expect(screen.getByRole('table', { name: 'Scripts that differ from the remote copy' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'Name' })).toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([{ kind: 'scripts' }]);
    expect(calls(invoke, 'memory_notes_copy_rows').every((args) => (args as { kind: string }).kind === 'scripts')).toBe(true);

    const boxes = screen.getAllByRole('checkbox');
    expect(boxes).toHaveLength(2);
    expect(boxes.every((box) => box.getAttribute('aria-checked') === 'false')).toBe(true);

    await fireEvent.click(screen.getByRole('button', { name: 'Not now' }));

    expect(onclose).toHaveBeenCalledOnce();
    expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(0);
  });

  it('sends only the ticked conflicts, with the kind', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    await openDialog();
    await screen.findByRole('table', { name: 'Scripts that differ from the remote copy' });

    const [first] = screen.getAllByRole('checkbox');
    const name = (first.getAttribute('aria-label') ?? '').replace('Overwrite the remote copy of ', '');
    const id = localScripts().find((script) => script.name === name)?.id;

    await fireEvent.click(pickBox(name));
    expect(screen.getByText('1 of 2 will overwrite the remote copy')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ kind: 'scripts', overwrite: [id] });
  });

  it('overwrites nothing when nothing is ticked', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toEqual(expect.objectContaining({ kind: 'scripts', overwrite: 'none' }));
  });

  it('deletes exactly the scripts the copy left on this computer, not one saved after the comparison', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withLocal());
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    const dialog = await screen.findByRole('dialog', { name: 'Copy finished' });
    const stays = within(dialog).getByText('Stay on this computer').nextElementSibling?.textContent;
    expect(Number(stays)).toBeGreaterThan(0);
    expect(toasts.items.at(-1)).toMatchObject({ tone: 'success', description: `${stays} scripts stay on this computer.` });

    const leftover = ((await invoke.mock.results.find((_, index) => invoke.mock.calls[index][0] === 'memory_notes_copy_apply')?.value) as NotesCopyResult).leftover;
    expect(leftover).toHaveLength(Number(stays));
    const [late] = localScripts();
    const saved = await invoke('memory_script_save', {
      project: null,
      draft: { name: 'saved-after', description: late.description, language: 'sh', scope: 'global', body: 'echo later', timeoutSecs: 30 },
      replace: false,
    });

    await fireEvent.click(within(dialog).getByRole('button', { name: `Delete ${stays} scripts from this computer…` }));
    expect(await screen.findByText(`Delete ${stays} scripts from this computer?`)).toBeInTheDocument();
    await fireEvent.click(await screen.findByRole('button', { name: 'Delete from this computer' }));

    await waitFor(() => expect(calls(invoke, 'memory_scripts_discard_local')).toEqual([{ ids: leftover }]));
    expect(calls(invoke, 'memory_notes_discard_local')).toHaveLength(0);
    expect(leftover).not.toContain((saved as { id: number }).id);
    expect(toasts.items.at(-1)).toMatchObject({ title: `${stays} scripts deleted from this computer` });
  });

  it('copies the notes with the kind of the notes', async () => {
    const invoke = fakeMemory();
    render(NotesCopyDialog, { open: true, onclose: vi.fn() });

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_apply')).toHaveLength(1));
    expect(calls(invoke, 'memory_notes_copy_preview')).toEqual([{ kind: 'notes' }]);
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ kind: 'notes' });
  });
});

describe('scripts the remote database blocks', () => {
  it('lists a script whose name is hidden on this computer with the reason, never tickable and not counted as copyable', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withBlockedScript());
    await openDialog();

    expect(await screen.findByText('Copy 5 scripts saved on this computer to the remote database?')).toBeInTheDocument();
    expect(screen.getByText('Blocked').nextElementSibling?.textContent).toBe('1');

    const table = await screen.findByRole('table', { name: 'Scripts that cannot be copied' });
    expect(within(table).getByText('local-script-3')).toBeInTheDocument();
    expect(within(table).getByText(/unhide it to copy this one/)).toBeInTheDocument();
    expect(within(table).getByRole('columnheader', { name: 'Name' })).toBeInTheDocument();
    expect(within(table).queryAllByRole('checkbox')).toHaveLength(0);
    expect(calls(invoke, 'memory_notes_copy_rows')).toContainEqual(expect.objectContaining({ kind: 'scripts', filter: 'failed' }));
  });

  it('keeps a blocked script out of what was copied and out of what is offered for deletion', async () => {
    const invoke = fakeMemory({ kind: 'available' }, withBlockedScript());
    await openDialog();

    await fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));

    const dialog = await screen.findByRole('dialog', { name: 'Copy finished' });
    const result = (await invoke.mock.results.find((_, index) => invoke.mock.calls[index][0] === 'memory_notes_copy_apply')?.value) as NotesCopyResult;
    const stat = (label: string) => within(dialog).getByText(label).nextElementSibling?.textContent;

    expect(result.failed).toEqual([expect.objectContaining({ localId: 103, title: 'local-script-3' })]);
    expect(result.leftover).not.toContain(103);
    expect(stat('Copied or already there')).toBe(String(result.counts.added + result.counts.identical));
    expect(stat('Failed')).toBe('1');
    expect(calls(invoke, 'memory_notes_copy_apply')[0]).toMatchObject({ kind: 'scripts', overwrite: 'none' });
  });
});
