import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

const TRUST = 'Run remote routines without asking';
const TRUST_HOOKS = 'Run remote hooks without asking';

/** Picks where an item is kept in the segmented row of that name: "Remote + this computer" shares it, "This computer" does not. */
async function keep(item: string, choice: 'Remote + this computer' | 'This computer'): Promise<void> {
  await fireEvent.click(within(await screen.findByRole('radiogroup', { name: item })).getByRole('radio', { name: choice }));
}

/** A backend that keeps the remote database settings in memory and answers the way the real one does, routine keys included. */
function fakeBackend(initial: Partial<MemoryRemote> = {}) {
  let state: MemoryRemote = {
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
    hooksSupported: true,
    shareHooks: false,
    trustTeamHooks: false,
    routinesSupported: true,
    shareRoutines: false,
    trustTeamRoutines: false,
    documentdbSupported: true,
    teamAndLocalSupported: true,
    scriptsTeamAndLocalSupported: true,
    ...initial,
  };
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_remote_get') {
      return state as CommandResult<K>;
    }

    if (name === 'memory_remote_set') {
      const next = args as CommandArgs<'memory_remote_set'>;

      state = { ...state, shareRoutines: next.shareRoutines ?? state.shareRoutines, trustTeamRoutines: next.trustTeamRoutines ?? state.trustTeamRoutines, urlRedacted: next.url === null ? state.urlRedacted : 'redacted://' + next.url };

      return state as CommandResult<K>;
    }

    return passthrough(name, args);
  });
}

function saves(invoke: ReturnType<typeof fakeBackend>): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === 'memory_remote_set').map(([, args]) => args);
}

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
  memoryRemote.value = null;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('The routines row of the remote database settings', () => {
  it('sits in Kept in after the hooks, with the same three choices', async () => {
    fakeBackend();
    render(SettingsView);

    await screen.findByRole('radiogroup', { name: 'Routines' });
    const rows = screen.getAllByRole('radiogroup').map((group) => group.getAttribute('aria-label'));

    expect(rows.filter((row) => ['Repository notes', 'Global notes', 'Scripts', 'Hooks', 'Routines'].includes(row ?? ''))).toEqual(['Repository notes', 'Global notes', 'Scripts', 'Hooks', 'Routines']);
    expect(within(screen.getByRole('radiogroup', { name: 'Routines' })).getAllByRole('radio').map((radio) => radio.textContent?.trim())).toEqual(['Remote', 'This computer', 'Remote + this computer']);
  });

  it('is hidden while the installed Harness Memory has no remote routines, and nothing about routines is saved', async () => {
    const invoke = fakeBackend({ routinesSupported: false });
    render(SettingsView);

    await screen.findByRole('radiogroup', { name: 'Hooks' });
    expect(screen.queryByRole('radiogroup', { name: 'Routines' })).not.toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument();

    await keep('Hooks', 'Remote + this computer');
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ shareHooks: true });
    expect(saves(invoke)[0]).not.toHaveProperty('shareRoutines');
    expect(saves(invoke)[0]).not.toHaveProperty('trustTeamRoutines');
  });

  it('starts on This computer, with no trust question for routines', async () => {
    fakeBackend();
    render(SettingsView);

    const group = await screen.findByRole('radiogroup', { name: 'Routines' });

    expect(within(group).getByRole('radio', { name: 'This computer' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument();
  });

  it('asks whether to run remote routines without asking once they are on the Remote database, off, with a plain warning', async () => {
    fakeBackend();
    render(SettingsView);

    await keep('Routines', 'Remote + this computer');
    const trust = await screen.findByRole('switch', { name: TRUST });

    expect(trust).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByText(/Routines from the remote database run without asking you first\. Anyone who can write to the remote database can then run code on this computer\./)).toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: TRUST_HOOKS })).not.toBeInTheDocument();
  });

  it('saves both choices, and keeps the trust only while routines are on the Remote database', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await keep('Routines', 'Remote + this computer');
    await fireEvent.click(await screen.findByRole('switch', { name: TRUST }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ shareRoutines: true, trustTeamRoutines: true, shareHooks: false });
    await waitFor(() => expect(screen.getByRole('switch', { name: TRUST })).toHaveAttribute('aria-checked', 'true'));

    await keep('Routines', 'This computer');
    await waitFor(() => expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument());
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(2));
    expect(saves(invoke)[1]).toMatchObject({ shareRoutines: false, trustTeamRoutines: false });

    // Sharing again asks for the trust again.
    await keep('Routines', 'Remote + this computer');
    expect(await screen.findByRole('switch', { name: TRUST })).toHaveAttribute('aria-checked', 'false');
  });

  it('starts over when the database address changes, and is saved on only if the user turns it on again', async () => {
    const invoke = fakeBackend({ shareRoutines: true, trustTeamRoutines: true });
    render(SettingsView);

    const group = await screen.findByRole('radiogroup', { name: 'Routines' });
    expect(within(group).getByRole('radio', { name: 'Remote + this computer' })).toHaveAttribute('aria-checked', 'true');
    expect(await screen.findByRole('switch', { name: TRUST })).toHaveAttribute('aria-checked', 'true');

    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'postgres://other.example/mem' } });

    expect(within(group).getByRole('radio', { name: 'This computer' })).toHaveAttribute('aria-checked', 'true');
    await waitFor(() => expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ url: 'postgres://other.example/mem', shareRoutines: false, trustTeamRoutines: false });
  });
});

describe('The tray switch of Settings', () => {
  it('is off by default, with its one-line description', async () => {
    fakeBackend();
    render(SettingsView);

    const tray = await screen.findByRole('switch', { name: 'Keep running in the tray when the window is closed' });

    expect(tray).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByText('Routines keep running on their schedule while Harness Desktop stays in the tray.')).toBeInTheDocument();
  });

  it('saves the choice as keepRunningInTray', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await fireEvent.click(await screen.findByRole('switch', { name: 'Keep running in the tray when the window is closed' }));

    await waitFor(() => expect(prefs.value?.keepRunningInTray).toBe(true));
    expect(invoke.mock.calls.filter(([name]) => name === 'prefs_set')).toHaveLength(1);
    expect(invoke.mock.calls.find(([name]) => name === 'prefs_set')?.[1]).toMatchObject({ prefs: { keepRunningInTray: true } });
    await waitFor(() => expect(screen.getByRole('switch', { name: 'Keep running in the tray when the window is closed' })).toHaveAttribute('aria-checked', 'true'));
  });
});
