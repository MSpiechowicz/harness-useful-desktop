import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryNotesPage } from '$lib/ipc/bindings/MemoryNotesPage';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

/** A backend with a saved remote database that keeps what the form saves, and counts the notes saved on this computer as the real server does. */
function fakeBackend(initial: Partial<MemoryRemote> = {}, page: Partial<MemoryNotesPage> = {}) {
  let state = {
    enabled: true,
    kind: 'postgres',
    urlRedacted: 'postgres://db/mem',
    hasToken: false,
    configPath: '/home/me/.config/harness-memory/config.toml',
    problem: null,
    repositoryNotes: 'remoteLocal',
    globalNotes: 'remoteLocal',
    scripts: 'remote',
    scriptsSupported: true,
    trustTeamScripts: false,
    trustTeamScriptsSupported: true,
    hooksSupported: false,
    routinesSupported: false,
    documentdbSupported: true,
    teamAndLocalSupported: true,
    scriptsTeamAndLocalSupported: true,
    offlineFallback: true,
    offlineFallbackSupported: true,
    notesSyncSupported: true,
    scriptsSyncSupported: true,
    savePlaceSupported: true,
    remoteLocalSaves: 'remote',
    remoteLocalSavesSupported: true,
    heldSavePlaceSupported: true,
    ...initial,
  } as MemoryRemote;
  const notes = { availability: { kind: 'available' }, total: 0, kinds: [], scopes: null, places: null, authors: null, combinedFilters: true, notes: [], hiddenCount: null, localCounts: { repo: 0, global: 0 }, offline: false, cachedAt: null, ...page } as MemoryNotesPage;
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_remote_get') {
      return state as CommandResult<K>;
    }

    if (name === 'memory_remote_set') {
      const next = args as CommandArgs<'memory_remote_set'>;

      state = { ...state, repositoryNotes: next.repositoryNotes, globalNotes: next.globalNotes, scripts: next.scripts, offlineFallback: next.offlineFallback ?? state.offlineFallback, remoteLocalSaves: next.remoteLocalSaves ?? state.remoteLocalSaves };

      return state as CommandResult<K>;
    }

    if (name === 'memory_notes_list') {
      return notes as CommandResult<K>;
    }

    return passthrough(name, args);
  });
}

const calls = (invoke: ReturnType<typeof fakeBackend>, name: string) => invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);

/** Makes the app's preferences refuse to be stored, once the test has set up what it needs. */
function refusePrefs(invoke: ReturnType<typeof fakeBackend>): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    if (name === 'prefs_set') {
      throw IpcFailure.from({ code: 'io', message: 'disk is full' });
    }

    return real(name as never, args);
  }) as typeof ipc.invoke);
}

/** Makes the Harness Memory settings refuse to be stored, once the test has set up what it needs. */
function refuseRemoteSet(invoke: ReturnType<typeof fakeBackend>): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    if (name === 'memory_remote_set') {
      throw IpcFailure.from({ code: 'io', message: 'config is read-only' });
    }

    return real(name as never, args);
  }) as typeof ipc.invoke);
}

const toggle = (name: string) => screen.findByRole('switch', { name });
const saveButton = () => screen.getByRole('button', { name: 'Save' });

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
  memoryRemote.value = null;
  toasts.items = [];
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('the copy switches of Remote memory', () => {
  it('are off until turned on, and each is saved with the form as a preference of the app', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    const toRemote = await toggle('Copy to remote');
    const toLocal = await toggle('Copy to this computer');

    expect(toRemote).toHaveAttribute('aria-checked', 'false');
    expect(toLocal).toHaveAttribute('aria-checked', 'false');
    expect(saveButton()).toBeDisabled();

    await fireEvent.click(toRemote);
    expect(saveButton()).toBeEnabled();
    expect(prefs.value?.memoryCopyToRemote).toBe(false);

    await fireEvent.click(saveButton());

    await waitFor(() => expect(prefs.value).toMatchObject({ memoryCopyToRemote: true, memoryCopyToLocal: false }));
    expect(calls(invoke, 'prefs_set')).toHaveLength(1);
    expect(calls(invoke, 'prefs_set')[0]).toMatchObject({ prefs: { memoryCopyToRemote: true, memoryCopyToLocal: false } });
    expect(calls(invoke, 'memory_remote_set')[0]).not.toHaveProperty('offlineFallback');
  });

  it('are saved on their own, and the form leaves the other preference as it was', async () => {
    const invoke = fakeBackend();
    prefs.value = samplePrefs({ motion: 'off', memoryCopyToRemote: true });
    render(SettingsView);

    expect(await toggle('Copy to remote')).toHaveAttribute('aria-checked', 'true');
    await fireEvent.click(await toggle('Copy to this computer'));
    await fireEvent.click(saveButton());

    await waitFor(() => expect(prefs.value).toMatchObject({ memoryCopyToRemote: true, memoryCopyToLocal: true }));
    expect(calls(invoke, 'prefs_set')).toHaveLength(1);
  });

  it('are greyed out with the reason while the installed Harness Memory cannot read both places', async () => {
    fakeBackend({ teamAndLocalSupported: false, scriptsTeamAndLocalSupported: false });
    render(SettingsView);

    expect(await toggle('Copy to remote')).toBeDisabled();
    expect(screen.getByRole('switch', { name: 'Copy to this computer' })).toBeDisabled();
    expect(screen.getAllByText('Update Harness Memory to copy between the remote and this computer.').length).toBeGreaterThan(0);
  });
});

describe('the copy switches while the installed server cannot run the copies on request', () => {
  it('are greyed out with the reason when it can keep both places but not copy', async () => {
    fakeBackend({ notesSyncSupported: false, scriptsSyncSupported: false });
    render(SettingsView);

    expect(await toggle('Copy to remote')).toBeDisabled();
    expect(screen.getByRole('switch', { name: 'Copy to this computer' })).toBeDisabled();
    expect(screen.getAllByText('Update Harness Memory to copy between the remote and this computer.').length).toBeGreaterThan(0);

    const notice = screen.getAllByText('Update Harness Memory to copy between the remote and this computer.').map((text) => text.closest('[role="status"]')).find(Boolean) as HTMLElement;
    expect(within(notice).getByRole('button')).toBeInTheDocument();
  });

  it('stay available while the scripts can still be copied', async () => {
    fakeBackend({ notesSyncSupported: false });
    render(SettingsView);

    expect(await toggle('Copy to remote')).toBeEnabled();
    expect(screen.getByRole('switch', { name: 'Copy to this computer' })).toBeEnabled();
  });
});

describe('saving the copy switches', () => {
  it('does not say saved while the preferences could not be stored, and keeps the form changed', async () => {
    const invoke = fakeBackend();
    refusePrefs(invoke);
    render(SettingsView);

    await fireEvent.click(await toggle('Copy to remote'));
    await fireEvent.click(saveButton());

    await waitFor(() => expect(toasts.items.map((toast) => toast.description)).toContain('disk is full'));
    await waitFor(() => expect(saveButton()).toBeEnabled());

    expect(screen.queryByText('Saved. Remote memory is on.')).toBeNull();
    expect(prefs.value?.memoryCopyToRemote).toBe(false);
    expect(screen.getByRole('switch', { name: 'Copy to remote' })).toHaveAttribute('aria-checked', 'true');
  });

  it('says saved once the preferences are stored on a second try', async () => {
    const invoke = fakeBackend();
    refusePrefs(invoke);
    render(SettingsView);

    await fireEvent.click(await toggle('Copy to remote'));
    await fireEvent.click(saveButton());
    await waitFor(() => expect(toasts.items.map((toast) => toast.description)).toContain('disk is full'));
    await waitFor(() => expect(saveButton()).toBeEnabled());

    invoke.mockRestore();
    fakeBackend();
    await fireEvent.click(saveButton());

    expect(await screen.findByText('Saved. Remote memory is on.')).toBeInTheDocument();
    expect(prefs.value?.memoryCopyToRemote).toBe(true);
  });
});

describe('saving on this computer while the remote cannot be reached', () => {
  it('is on by default, and its description warns that notes can be lost without it', async () => {
    fakeBackend();
    render(SettingsView);

    expect(await toggle('Save on this computer when the remote is unreachable')).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByText(/notes saved during an outage can be lost/)).toBeInTheDocument();
  });

  it('sends the choice with the form only when it was changed', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await fireEvent.click(await toggle('Save on this computer when the remote is unreachable'));
    await fireEvent.click(saveButton());

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).toMatchObject({ offlineFallback: false });
    expect(await toggle('Save on this computer when the remote is unreachable')).toHaveAttribute('aria-checked', 'false');
  });

  it('is greyed out with an inline update while the installed Harness Memory does not know it, and is never sent', async () => {
    const invoke = fakeBackend({ offlineFallbackSupported: false });
    render(SettingsView);

    expect(await toggle('Save on this computer when the remote is unreachable')).toBeDisabled();
    const notice = screen.getByText('Update Harness Memory to save on this computer when the remote is unreachable.').closest('[role="status"]') as HTMLElement;
    expect(within(notice).getByRole('button')).toBeInTheDocument();

    await fireEvent.click(await toggle('Copy to remote'));
    await fireEvent.click(saveButton());

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).not.toHaveProperty('offlineFallback');
  });
});

describe('where new notes and scripts save to', () => {
  const ROW = 'New notes and scripts save to';
  /** The choices of the row: the routing rows above it have segments of the same names. */
  const group = async () => within(await screen.findByRole('radiogroup', { name: ROW }));
  const choice = async (name: string) => (await group()).getByRole('radio', { name });

  it('starts on the saved choice, says it covers agents, and is stored with the Harness Memory settings', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    expect(await choice('Remote')).toBeChecked();
    expect((await group()).getByRole('radio', { name: 'This computer' })).not.toBeChecked();
    expect(screen.getAllByText(/for agents and for what you add/).length).toBeGreaterThan(0);
    expect(screen.queryByText(/agents save are not affected/)).toBeNull();
    expect(saveButton()).toBeDisabled();

    await fireEvent.click(await choice('This computer'));
    expect(saveButton()).toBeEnabled();

    await fireEvent.click(saveButton());

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).toMatchObject({ remoteLocalSaves: 'local' });
    expect(calls(invoke, 'prefs_set')).toHaveLength(0);
    expect(await screen.findByText('Saved. Remote memory is on.')).toBeInTheDocument();
    expect(await choice('This computer')).toBeChecked();
  });

  it('shows the stored choice', async () => {
    fakeBackend({ remoteLocalSaves: 'local' });
    render(SettingsView);

    expect(await choice('This computer')).toBeChecked();
    expect(await choice('Remote')).not.toBeChecked();
  });

  it('is sent only when it was changed', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await fireEvent.click(await toggle('Copy to remote'));
    await fireEvent.click(saveButton());

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).not.toHaveProperty('remoteLocalSaves');
  });

  it('shows for scripts alone kept in Remote + this computer', async () => {
    fakeBackend({ repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remoteLocal' });
    render(SettingsView);

    expect(await choice('Remote')).toBeInTheDocument();
  });

  it('is hidden while no scope is kept in Remote + this computer, and is not sent', async () => {
    const invoke = fakeBackend({ repositoryNotes: 'remote', globalNotes: 'local', scripts: 'remote', remoteLocalSaves: 'local' });
    render(SettingsView);

    await fireEvent.click(await toggle('Copy to remote'));

    expect(screen.queryByRole('radiogroup', { name: ROW })).toBeNull();

    await fireEvent.click(saveButton());
    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).not.toHaveProperty('remoteLocalSaves');
  });

  it('is greyed out with an inline update while the installed Harness Memory does not know it, and is never sent', async () => {
    const invoke = fakeBackend({ remoteLocalSavesSupported: false });
    render(SettingsView);

    expect((await choice('Remote'))).toBeDisabled();
    expect((await choice('This computer'))).toBeDisabled();
    const notice = screen.getAllByText('Update Harness Memory to choose where new notes and scripts are saved.').map((text) => text.closest('[role="status"]')).find(Boolean) as HTMLElement;
    expect(within(notice).getByRole('button')).toBeInTheDocument();

    await fireEvent.click(await toggle('Copy to remote'));
    await fireEvent.click(saveButton());

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).not.toHaveProperty('remoteLocalSaves');
  });

  it('does not say saved while the settings could not be stored, and keeps the form changed', async () => {
    const invoke = fakeBackend();
    refuseRemoteSet(invoke);
    render(SettingsView);

    await fireEvent.click(await choice('This computer'));
    await fireEvent.click(saveButton());

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    await waitFor(() => expect(saveButton()).toBeEnabled());

    expect(screen.queryByText('Saved. Remote memory is on.')).toBeNull();
    expect(await choice('This computer')).toBeChecked();
  });
});
