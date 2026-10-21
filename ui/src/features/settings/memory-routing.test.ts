import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryNotesPage } from '$lib/ipc/bindings/MemoryNotesPage';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemoryScriptsPage } from '$lib/ipc/bindings/MemoryScriptsPage';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

const CHANGE = 'Change where memory is kept?';

/** A backend that keeps the settings in memory and counts the notes and scripts saved on this computer, the way a server with a remote database does. */
function fakeBackend(initial: Partial<MemoryRemote> = {}, localCounts = { repo: 0, global: 0 }, scriptCounts = { repo: 0, global: 0 }) {
  let state = {
    enabled: true,
    kind: 'postgres',
    urlRedacted: 'postgres://db/mem',
    hasToken: false,
    configPath: '/home/me/.config/harness-memory/config.toml',
    problem: null,
    repositoryNotes: 'remote',
    globalNotes: 'remote',
    scripts: 'remote',
    scriptsSupported: true,
    trustTeamScripts: false,
    trustTeamScriptsSupported: true,
    hooksSupported: false,
    shareHooks: false,
    trustTeamHooks: false,
    routinesSupported: false,
    shareRoutines: false,
    trustTeamRoutines: false,
    documentdbSupported: true,
    teamAndLocalSupported: true,
    scriptsTeamAndLocalSupported: true,
    ...initial,
  } as MemoryRemote;
  const page = { availability: { kind: 'available' }, total: 0, kinds: [], scopes: null, places: null, combinedFilters: true, notes: [], hiddenCount: null, localCounts } as MemoryNotesPage;
  const scriptsPage = { availability: { kind: 'available' }, total: 0, groups: { needsReview: 0, repo: 0, global: 0 }, scopes: null, review: null, places: null, combinedFilters: true, scripts: [], hiddenCount: null, localCounts: scriptCounts } as MemoryScriptsPage;
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_remote_get') {
      return state as CommandResult<K>;
    }

    if (name === 'memory_remote_set') {
      const next = args as CommandArgs<'memory_remote_set'>;

      state = { ...state, repositoryNotes: next.repositoryNotes, globalNotes: next.globalNotes, scripts: next.scripts };

      return state as CommandResult<K>;
    }

    if (name === 'memory_notes_list') {
      return page as CommandResult<K>;
    }

    if (name === 'memory_scripts_list') {
      return scriptsPage as CommandResult<K>;
    }

    // The copy dialog loads its own preview: it never answers here, which keeps the dialog on its first stage.
    if (name === 'memory_notes_copy_preview') {
      return new Promise<CommandResult<K>>(() => {});
    }

    return passthrough(name, args);
  });
}

/** The kind of thing each copy dialog asked to preview, in order: notes unless the call says scripts. */
function kindsOf(invoke: ReturnType<typeof fakeBackend>): unknown[] {
  return calls(invoke, 'memory_notes_copy_preview').map(([, args]) => (args as { kind?: string } | undefined)?.kind ?? 'notes');
}

function calls(invoke: ReturnType<typeof fakeBackend>, name: string) {
  return invoke.mock.calls.filter(([command]) => command === name);
}

function saves(invoke: ReturnType<typeof fakeBackend>): unknown[] {
  return calls(invoke, 'memory_remote_set').map(([, args]) => args);
}

async function choose(group: 'Repository notes' | 'Global notes' | 'Scripts', place: string): Promise<void> {
  await fireEvent.click(within(await screen.findByRole('radiogroup', { name: group })).getByRole('radio', { name: place }));
}

/** Leaves the copy dialog the way Not now does: its preview never answers here, so it has no buttons yet, but Escape closes it. */
async function dismissCopyDialog(): Promise<void> {
  await fireEvent.keyDown(await screen.findByRole('dialog'), { key: 'Escape' });
}

async function saveForm(): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
}

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
  memoryRemote.value = null;
  viewState.noteId = null;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('The third place for notes, team plus this computer', () => {
  it('is offered for repository and global notes when Harness Memory can do it', async () => {
    fakeBackend();
    render(SettingsView);

    for (const group of ['Repository notes', 'Global notes'] as const) {
      const radios = within(await screen.findByRole('radiogroup', { name: group })).getAllByRole('radio');

      expect(radios.map((radio) => radio.textContent?.trim())).toEqual(['Remote', 'This computer', 'Remote + this computer']);
      expect(radios[2]).toBeEnabled();
    }

    expect(screen.queryByText('Update Harness Memory to read remote notes and save new ones on this computer.')).not.toBeInTheDocument();
  });

  it('is greyed out with an inline update while the installed Harness Memory cannot do it, and the other places still work', async () => {
    fakeBackend({ teamAndLocalSupported: false });
    render(SettingsView);

    const repository = await screen.findByRole('radiogroup', { name: 'Repository notes' });

    expect(within(repository).getByRole('radio', { name: 'Remote + this computer' })).toBeDisabled();
    expect(within(screen.getByRole('radiogroup', { name: 'Global notes' })).getByRole('radio', { name: 'Remote + this computer' })).toBeDisabled();
    expect(within(repository).getByRole('radio', { name: 'This computer' })).toBeEnabled();
    expect(screen.getByText('Update Harness Memory to read remote notes and save new ones on this computer.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update Harness Memory' })).toBeEnabled();
  });

  it('offers the scripts the same three places, and saves reading both when the user chooses them', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    const radios = within(await screen.findByRole('radiogroup', { name: 'Scripts' })).getAllByRole('radio');

    expect(radios.map((radio) => radio.textContent?.trim())).toEqual(['Remote', 'This computer', 'Remote + this computer']);
    expect(radios[2]).toBeEnabled();
    expect(screen.queryByText('Update Harness Memory to read remote and this computer together for scripts.')).not.toBeInTheDocument();

    await choose('Scripts', 'Remote + this computer');
    await saveForm();
    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getByText('Nothing is hidden. New scripts are saved on this computer and remote scripts become read-only here.')).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ scripts: 'remoteLocal', repositoryNotes: 'remote' });
  });

  it('greys the scripts place that reads both out with an inline update while only that is missing, and the notes keep theirs', async () => {
    fakeBackend({ scriptsTeamAndLocalSupported: false });
    render(SettingsView);

    const scripts = await screen.findByRole('radiogroup', { name: 'Scripts' });

    expect(within(scripts).getByRole('radio', { name: 'Remote + this computer' })).toBeDisabled();
    expect(within(scripts).getByRole('radio', { name: 'Remote' })).toBeEnabled();
    expect(within(scripts).getByRole('radio', { name: 'This computer' })).toBeEnabled();
    expect(within(screen.getByRole('radiogroup', { name: 'Repository notes' })).getByRole('radio', { name: 'Remote + this computer' })).toBeEnabled();
    expect(screen.getByText('Update Harness Memory to read remote and this computer together for scripts.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update Harness Memory' })).toBeEnabled();
  });

  it('keeps asking whether to run remote scripts without asking while the scripts also read this computer', async () => {
    const invoke = fakeBackend({ scripts: 'remoteLocal' });
    render(SettingsView);

    await fireEvent.click(await screen.findByRole('switch', { name: 'Run remote scripts without asking' }));
    await saveForm();

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ scripts: 'remoteLocal', trustTeamScripts: true });
    expect(screen.queryByRole('dialog', { name: CHANGE })).not.toBeInTheDocument();

    await choose('Scripts', 'This computer');
    await waitFor(() => expect(screen.queryByRole('switch', { name: 'Run remote scripts without asking' })).not.toBeInTheDocument());
  });

  it('says what each place means in the row, and saves it after the user has read what changes', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await choose('Repository notes', 'Remote + this computer');
    await saveForm();

    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getByText('Repository notes')).toBeInTheDocument();
    expect(within(dialog).getByText('Nothing is hidden. New notes are saved on this computer and remote notes become read-only here.')).toBeInTheDocument();
    expect(saves(invoke)).toHaveLength(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ repositoryNotes: 'remoteLocal', globalNotes: 'remote' });
  });

  it('keeps the settings and the open note when the user cancels', async () => {
    const invoke = fakeBackend();
    viewState.noteId = 42;
    render(SettingsView);

    await choose('Global notes', 'This computer');
    await saveForm();
    await fireEvent.click(within(await screen.findByRole('dialog', { name: CHANGE })).getByRole('button', { name: 'Cancel' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: CHANGE })).not.toBeInTheDocument());
    expect(saves(invoke)).toHaveLength(0);
    expect(viewState.noteId).toBe(42);
  });

  it('closes the open note once the routing is saved, since note ids shift', async () => {
    const invoke = fakeBackend();
    viewState.noteId = 42;
    render(SettingsView);

    await choose('Repository notes', 'This computer');
    await saveForm();
    await fireEvent.click(within(await screen.findByRole('dialog', { name: CHANGE })).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    await waitFor(() => expect(viewState.noteId).toBeNull());
  });

  it('asks about the scripts alone when only the scripts choice changes, in the words of scripts', async () => {
    const invoke = fakeBackend();
    viewState.noteId = 42;
    viewState.scriptId = 8;
    render(SettingsView);

    await choose('Scripts', 'This computer');
    await saveForm();

    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getAllByRole('listitem')).toHaveLength(1);
    expect(within(dialog).getByText('Scripts')).toBeInTheDocument();
    expect(within(dialog).getByText('Remote scripts are hidden on this computer. They stay in the remote database.')).toBeInTheDocument();
    expect(saves(invoke)).toHaveLength(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ scripts: 'local', repositoryNotes: 'remote' });
    await waitFor(() => expect(viewState.scriptId).toBeNull());
    expect(viewState.noteId).toBeNull();
  });

  it('saves trusting remote scripts as off when they move to this computer, even if it was on before', async () => {
    const invoke = fakeBackend({ trustTeamScripts: true });
    render(SettingsView);

    await choose('Scripts', 'This computer');
    await saveForm();
    await fireEvent.click(within(await screen.findByRole('dialog', { name: CHANGE })).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toEqual({ enabled: true, kind: 'postgres', url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'local', trustTeamScripts: false });
  });

  it('keeps the open script when the scripts choice is not saved', async () => {
    fakeBackend();
    viewState.scriptId = 8;
    render(SettingsView);

    await choose('Scripts', 'Remote + this computer');
    await saveForm();
    await fireEvent.click(within(await screen.findByRole('dialog', { name: CHANGE })).getByRole('button', { name: 'Cancel' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: CHANGE })).not.toBeInTheDocument());
    expect(viewState.scriptId).toBe(8);
  });

  it('counts the scripts saved on this computer, in every repository and the global ones, when they move back to the remote database', async () => {
    const invoke = fakeBackend({ scripts: 'local' }, undefined, { repo: 3, global: 2 });
    render(SettingsView);

    await choose('Scripts', 'Remote');
    await saveForm();

    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getByText('5 scripts saved on this computer are hidden. You can copy them to the remote database next.')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ scripts: 'remote' });
  });

  it('offers to copy the notes first and the scripts after, one dialog at a time, when one save moves both to the remote database', async () => {
    const invoke = fakeBackend({ repositoryNotes: 'local', scripts: 'local' }, { repo: 3, global: 0 }, { repo: 1, global: 1 });
    render(SettingsView);

    await choose('Repository notes', 'Remote');
    await choose('Scripts', 'Remote');
    await saveForm();
    const change = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(change).getAllByRole('listitem').map((item) => within(item).getAllByRole('paragraph')[0].textContent)).toEqual(['Repository notes', 'Scripts']);
    await fireEvent.click(within(change).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_preview')).toHaveLength(1));
    expect(kindsOf(invoke)).toEqual(['notes']);
    await dismissCopyDialog();

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_preview')).toHaveLength(2));
    expect(kindsOf(invoke)).toEqual(['notes', 'scripts']);
    await dismissCopyDialog();

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_notes_copy_preview')).toHaveLength(2);
  });

  it('offers only the scripts when only they have something saved on this computer', async () => {
    const invoke = fakeBackend({ repositoryNotes: 'local', scripts: 'local' }, { repo: 0, global: 0 }, { repo: 2, global: 0 });
    render(SettingsView);

    await choose('Repository notes', 'Remote');
    await choose('Scripts', 'Remote');
    await saveForm();
    await fireEvent.click(within(await screen.findByRole('dialog', { name: CHANGE })).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_preview')).toHaveLength(1));
    expect(kindsOf(invoke)).toEqual(['scripts']);
  });

  it('counts the notes saved on this computer, across all repositories, when moving back to the remote database', async () => {
    const invoke = fakeBackend({ repositoryNotes: 'local' }, { repo: 3, global: 0 });
    render(SettingsView);

    await choose('Repository notes', 'Remote');
    await saveForm();

    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getByText('3 notes saved on this computer, across all repositories, are hidden. You can copy them to the remote database next.')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ repositoryNotes: 'remote' });
  });

  it('offers to copy the notes saved on this computer once a scope is back in the remote database', async () => {
    const invoke = fakeBackend({ repositoryNotes: 'local' }, { repo: 3, global: 0 });
    render(SettingsView);

    await choose('Repository notes', 'Remote');
    await saveForm();
    await fireEvent.click(within(await screen.findByRole('dialog', { name: CHANGE })).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_notes_copy_preview')).toHaveLength(1));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });

  it('offers nothing when no note is saved on this computer', async () => {
    const invoke = fakeBackend({ repositoryNotes: 'local' }, { repo: 0, global: 5 });
    render(SettingsView);

    await choose('Repository notes', 'Remote');
    await saveForm();

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    await screen.findByText('Saved. Remote memory is on.');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(calls(invoke, 'memory_notes_copy_preview')).toHaveLength(0);
  });

  it('does not offer to copy after moving to this computer', async () => {
    const invoke = fakeBackend({}, { repo: 3, global: 0 });
    render(SettingsView);

    await choose('Repository notes', 'This computer');
    await saveForm();
    await fireEvent.click(within(await screen.findByRole('dialog', { name: CHANGE })).getByRole('button', { name: 'Save' }));

    await screen.findByText('Saved. Remote memory is on.');
    expect(calls(invoke, 'memory_notes_copy_preview')).toHaveLength(0);
  });
});
