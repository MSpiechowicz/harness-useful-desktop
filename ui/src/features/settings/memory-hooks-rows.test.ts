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

const TRUST = 'Run remote hooks without asking';
const TRUST_SCRIPTS = 'Run remote scripts without asking';

/** Picks where an item is kept in the segmented row of that name: "Remote + this computer" shares it, "This computer" does not. */
async function keep(item: string, choice: 'Remote + this computer' | 'This computer'): Promise<void> {
  await fireEvent.click(within(await screen.findByRole('radiogroup', { name: item })).getByRole('radio', { name: choice }));
}

/** A backend that keeps the remote database settings in memory and answers the way the real one does, hook keys included. */
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
    saveGuardsSupported: false,
    embedderSupported: false,
    settingsSupported: false,
    notesLangSupported: false,
    ...initial,
  };
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_remote_get') {
      return state as CommandResult<K>;
    }

    if (name === 'memory_remote_set') {
      const next = args as CommandArgs<'memory_remote_set'>;

      state = { ...state, shareHooks: next.shareHooks ?? state.shareHooks, trustTeamHooks: next.trustTeamHooks ?? state.trustTeamHooks, urlRedacted: next.url === null ? state.urlRedacted : 'redacted://' + next.url };

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

describe('The panel of the remote database settings', () => {
  it('has the sections Connection, Kept in and Trust, in that order', async () => {
    fakeBackend({ shareHooks: true });
    render(SettingsView);

    await screen.findByRole('radiogroup', { name: 'Hooks' });
    const headings = screen.getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent);

    expect(headings.filter((text) => ['Connection', 'Kept in', 'Trust'].includes(text ?? ''))).toEqual(['Connection', 'Kept in', 'Trust']);
    const places = (item: string) => within(screen.getByRole('radiogroup', { name: item })).getAllByRole('radio').map((radio) => radio.textContent?.trim());

    // Every row of Kept in offers the same three places, in the same order.
    for (const item of ['Project notes', 'Global notes', 'Scripts', 'Hooks', 'Routines']) {
      expect(places(item)).toEqual(['Remote', 'This computer', 'Remote + this computer']);
    }
  });
});

describe('The Remote place of the hooks and routines rows', () => {
  it.each([
    ['Hooks', 'Hooks you make always run on this computer, so Remote alone does not apply.'],
    ['Routines', 'Routines you make always run on this computer, so Remote alone does not apply.'],
  ])('is greyed out on the %s row, and says why when the user points at it', async (item, reason) => {
    fakeBackend();
    render(SettingsView);

    const remoteOnly = within(await screen.findByRole('radiogroup', { name: item })).getByRole('radio', { name: 'Remote' });

    expect(remoteOnly).toBeDisabled();
    expect(within(screen.getByRole('radiogroup', { name: 'Scripts' })).getByRole('radio', { name: 'Remote' })).toBeEnabled();

    await fireEvent.pointerEnter(remoteOnly.parentElement!);

    expect(await screen.findByRole('tooltip')).toHaveTextContent(reason);
  });

  it.each([
    ['Hooks', 'This computer', 'shareHooks', false],
    ['Hooks', 'Remote + this computer', 'shareHooks', true],
    ['Routines', 'This computer', 'shareRoutines', false],
    ['Routines', 'Remote + this computer', 'shareRoutines', true],
  ] as const)('still saves %s on %s as %s %s', async (item, place, key, shared) => {
    const invoke = fakeBackend({ shareHooks: !shared, shareRoutines: !shared });
    render(SettingsView);

    await keep(item, place);
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ [key]: shared });
  });

  it('shows what is shared as Remote + this computer, and what is not as This computer', async () => {
    fakeBackend({ shareHooks: true, shareRoutines: false });
    render(SettingsView);

    expect(within(await screen.findByRole('radiogroup', { name: 'Hooks' })).getByRole('radio', { name: 'Remote + this computer' })).toBeChecked();
    expect(within(screen.getByRole('radiogroup', { name: 'Routines' })).getByRole('radio', { name: 'This computer' })).toBeChecked();
  });
});

describe('The hooks row of the remote database settings', () => {
  it('is hidden while the installed Harness Memory has no remote hooks, and nothing about hooks is saved', async () => {
    const invoke = fakeBackend({ hooksSupported: false });
    render(SettingsView);

    await screen.findByRole('switch', { name: TRUST_SCRIPTS });
    expect(screen.queryByRole('radiogroup', { name: 'Hooks' })).not.toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('switch', { name: TRUST_SCRIPTS }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).not.toHaveProperty('shareHooks');
    expect(saves(invoke)[0]).not.toHaveProperty('trustTeamHooks');
  });

  it('starts on This computer, with no trust question for hooks', async () => {
    fakeBackend();
    render(SettingsView);

    const group = await screen.findByRole('radiogroup', { name: 'Hooks' });

    expect(within(group).getByRole('radio', { name: 'This computer' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument();
  });

  it('shows the hooks trust question once hooks are on the Remote database, off, with a plain warning', async () => {
    fakeBackend();
    render(SettingsView);

    await keep('Hooks', 'Remote + this computer');
    const trust = await screen.findByRole('switch', { name: TRUST });

    expect(trust).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByText(/Hooks from the remote database run without asking you first\. Anyone who can write to the remote database can then run code on this computer\./)).toBeInTheDocument();
  });

  it('keeps each trust question to its own item: scripts on This computer hide only the scripts one', async () => {
    fakeBackend({ shareHooks: true });
    render(SettingsView);

    await screen.findByRole('switch', { name: TRUST });
    await keep('Scripts', 'This computer');

    await waitFor(() => expect(screen.queryByRole('switch', { name: TRUST_SCRIPTS })).not.toBeInTheDocument());
    expect(screen.getByRole('switch', { name: TRUST })).toBeInTheDocument();

    await keep('Hooks', 'This computer');
    await waitFor(() => expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument());
    expect(screen.queryByRole('heading', { name: 'Trust' })).not.toBeInTheDocument();
  });

  it('saves both choices, and keeps the hook trust only while hooks are on the Remote database', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await keep('Hooks', 'Remote + this computer');
    await fireEvent.click(await screen.findByRole('switch', { name: TRUST }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ shareHooks: true, trustTeamHooks: true });
    await waitFor(() => expect(screen.getByRole('switch', { name: TRUST })).toHaveAttribute('aria-checked', 'true'));

    await keep('Hooks', 'This computer');
    await waitFor(() => expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument());
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(2));
    expect(saves(invoke)[1]).toMatchObject({ shareHooks: false, trustTeamHooks: false });

    // Sharing again asks for the trust again.
    await keep('Hooks', 'Remote + this computer');
    expect(await screen.findByRole('switch', { name: TRUST })).toHaveAttribute('aria-checked', 'false');
  });

  it('starts over when the database address or kind changes, and is saved on only if the user turns it on again', async () => {
    const invoke = fakeBackend({ shareHooks: true, trustTeamHooks: true });
    render(SettingsView);

    const group = await screen.findByRole('radiogroup', { name: 'Hooks' });
    expect(within(group).getByRole('radio', { name: 'Remote + this computer' })).toHaveAttribute('aria-checked', 'true');
    expect(await screen.findByRole('switch', { name: TRUST })).toHaveAttribute('aria-checked', 'true');

    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'postgres://other.example/mem' } });

    expect(within(group).getByRole('radio', { name: 'This computer' })).toHaveAttribute('aria-checked', 'true');
    await waitFor(() => expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ url: 'postgres://other.example/mem', shareHooks: false, trustTeamHooks: false });

    await keep('Hooks', 'Remote + this computer');
    await fireEvent.click(await screen.findByRole('switch', { name: TRUST }));
    await fireEvent.click(screen.getByRole('button', { name: 'Database: PostgreSQL' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Remote SQLite (libSQL, Turso)' }));

    expect(within(group).getByRole('radio', { name: 'This computer' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.queryByRole('switch', { name: TRUST })).not.toBeInTheDocument();
  });
});
