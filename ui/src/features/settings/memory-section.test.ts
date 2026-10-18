import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemoryRemoteTest } from '$lib/ipc/bindings/MemoryRemoteTest';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

const POSTGRES_URL = 'postgres://me:secret@db.team.example:5432/mem?sslmode=require';

/** A backend that keeps the remote-memory settings in memory and answers the way the real one does. */
function fakeBackend(initial: Partial<MemoryRemote> = {}, probe: MemoryRemoteTest = { ok: true, message: 'Connected to PostgreSQL 16.' }) {
  let state: MemoryRemote = {
    enabled: false,
    kind: null,
    urlRedacted: null,
    hasToken: false,
    configPath: '/home/me/.config/harness/memory-remote.json',
    problem: null,
    repositoryNotes: 'remote',
    globalNotes: 'remote',
    scripts: 'remote',
    scriptsSupported: true,
    trustTeamScripts: false,
    trustTeamScriptsSupported: true,
    hooksSupported: false, shareHooks: false, trustTeamHooks: false,
    ...initial,
  };
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_remote_get') {
      return state as CommandResult<K>;
    }

    if (name === 'memory_remote_set') {
      const next = args as CommandArgs<'memory_remote_set'>;

      state = {
        ...state,
        enabled: next.enabled,
        kind: next.kind,
        urlRedacted: next.url === null ? state.urlRedacted : 'redacted://' + next.url.split('@').pop(),
        hasToken: next.token === null ? state.hasToken : next.token !== '',
        repositoryNotes: next.repositoryNotes,
        globalNotes: next.globalNotes,
        scripts: next.scripts,
        trustTeamScripts: next.trustTeamScripts,
      };

      return state as CommandResult<K>;
    }

    if (name === 'memory_remote_test') {
      return probe as CommandResult<K>;
    }

    return passthrough(name, args);
  });
}

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
  memoryRemote.value = null;
  prefs.dismissMemoryManualRemovals();
});

afterEach(() => {
  vi.restoreAllMocks();
});

function memorySection(): HTMLElement {
  return screen.getByRole('region', { name: 'Memory' });
}

async function openPanel(): Promise<void> {
  const remote = await within(memorySection()).findByRole('switch', { name: 'Remote memory' });
  await waitFor(() => expect(remote).toBeEnabled());
  await fireEvent.click(remote);
}

function calls(invoke: ReturnType<typeof fakeBackend>, name: string): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);
}

describe('Memory section', () => {
  it('is its own entry in the section nav and holds the memory switches that used to be under Harnesses', async () => {
    fakeBackend();
    render(SettingsView);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });

    await fireEvent.click(within(nav).getByRole('button', { name: 'Memory' }));

    expect(within(nav).getByRole('button', { name: 'Memory' })).toHaveAttribute('aria-current', 'true');
    expect(Element.prototype.scrollIntoView).toHaveBeenCalled();

    const memory = memorySection();
    expect(within(memory).getByRole('switch', { name: 'Project memory' })).toBeInTheDocument();
    expect(within(memory).getByRole('switch', { name: 'Remote memory' })).toBeInTheDocument();

    const harnesses = screen.getByRole('region', { name: 'Harnesses' });
    expect(within(harnesses).queryByRole('switch', { name: 'Project memory' })).not.toBeInTheDocument();
  });

  it('finds the section by its search words', async () => {
    fakeBackend();
    render(SettingsView);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search settings' }), { target: { value: 'teammate' } });

    await waitFor(() => expect(within(nav).getAllByRole('button').map((button) => button.textContent?.trim())).toEqual(['Memory']));
  });

  it('warns which entry to remove by hand, with a copyable name, when turning memory off left one behind', async () => {
    const passthrough = ipc.invoke.bind(ipc);
    const removal = { harness: 'opencode', file: '~/.config/opencode/opencode.jsonc', name: 'harness-memory' } as const;
    vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
      if (name === 'memory_set_enabled') {
        const { enabled } = args as CommandArgs<'memory_set_enabled'>;

        return { memoryEnabled: enabled, manualRemovals: enabled ? [] : [removal] } as CommandResult<K>;
      }

      return passthrough(name, args);
    });
    render(SettingsView);

    await fireEvent.click(within(memorySection()).getByRole('switch', { name: 'Project memory' }));

    const note = await within(memorySection()).findByRole('alert', { name: 'Remove these entries by hand' });
    expect(note).toHaveTextContent('OpenCode has comments in ~/.config/opencode/opencode.jsonc');
    expect(within(note).getByText('harness-memory', { selector: 'code *, code' })).toBeInTheDocument();
    expect(within(note).getByRole('button', { name: 'Copy entry name' })).toBeInTheDocument();

    // Turning it back on has nothing left to remove: the note goes.
    await fireEvent.click(within(memorySection()).getByRole('switch', { name: 'Project memory' }));
    await waitFor(() => expect(screen.queryByRole('alert', { name: 'Remove these entries by hand' })).not.toBeInTheDocument());
  });

  it('shows no warning when turning memory off left nothing to remove', async () => {
    fakeBackend();
    render(SettingsView);

    await fireEvent.click(within(memorySection()).getByRole('switch', { name: 'Project memory' }));

    await waitFor(() => expect(within(memorySection()).getByRole('switch', { name: 'Project memory' })).toHaveAttribute('aria-checked', 'false'));
    expect(screen.queryByRole('alert', { name: 'Remove these entries by hand' })).not.toBeInTheDocument();
  });

  it('greys out a saved remote connection while project memory is off and keeps it for when it is back on', async () => {
    prefs.value = samplePrefs({ motion: 'off', memoryEnabled: false });
    fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://alice@db.team.example:5432/memory' });
    render(SettingsView);

    const remote = await within(memorySection()).findByRole('switch', { name: 'Remote memory' });

    expect(remote).toBeDisabled();
    expect(remote).toHaveAttribute('aria-checked', 'true');
    const url = screen.getByLabelText('Connection URL');
    expect(url.closest('[inert]')).not.toBeNull();
  });

  it('keeps the connection form hidden while remote memory is off', async () => {
    fakeBackend();
    render(SettingsView);

    const remote = await within(memorySection()).findByRole('switch', { name: 'Remote memory' });

    expect(remote).toHaveAttribute('aria-checked', 'false');
    expect(screen.queryByLabelText('Connection URL')).not.toBeInTheDocument();
  });

  it('shows the form when remote memory is switched on, with a placeholder and token field per database', async () => {
    fakeBackend();
    render(SettingsView);

    await openPanel();

    expect(await screen.findByRole('button', { name: 'Database: PostgreSQL' })).toBeInTheDocument();
    expect(screen.getByLabelText('Connection URL')).toHaveAttribute('placeholder', 'postgres://user:password@host:5432/db?sslmode=require');
    expect(screen.queryByLabelText('Auth token')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.click(screen.getByRole('button', { name: 'Database: PostgreSQL' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Remote SQLite (libSQL, Turso)' }));

    expect(screen.getByRole('button', { name: 'Database: Remote SQLite (libSQL, Turso)' })).toBeInTheDocument();
    expect(screen.getByLabelText('Connection URL')).toHaveAttribute('placeholder', 'libsql://team-memory-org.turso.io');
    expect(screen.getByLabelText('Auth token')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: /^Database:/ }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'MongoDB' }));

    expect(screen.getByLabelText('Connection URL')).toHaveAttribute('placeholder', 'mongodb+srv://user:password@cluster.example.net/harness_memory');
    expect(screen.queryByLabelText('Auth token')).not.toBeInTheDocument();
  });

  it('masks the URL until the eye button reveals it, and the eye is not a bordered button', async () => {
    fakeBackend();
    render(SettingsView);
    await openPanel();
    const url = await screen.findByLabelText('Connection URL');

    expect(url).toHaveAttribute('type', 'password');

    const eye = screen.getByRole('button', { name: 'Show URL' });
    expect(eye.className).not.toMatch(/\bborder\b/);

    await fireEvent.click(eye);

    expect(url).toHaveAttribute('type', 'text');
    expect(screen.getByRole('button', { name: 'Hide URL' })).toBeInTheDocument();
  });

  it('refuses a URL that does not fit the database and enables Save for one that does', async () => {
    fakeBackend();
    render(SettingsView);
    await openPanel();
    const url = await screen.findByLabelText('Connection URL');

    await fireEvent.input(url, { target: { value: 'mongodb://host/db' } });
    await fireEvent.blur(url);

    expect(await screen.findByText('A PostgreSQL URL starts with postgres:// or postgresql://.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Test connection' })).toBeDisabled();

    await fireEvent.input(url, { target: { value: POSTGRES_URL } });

    expect(screen.queryByText('A PostgreSQL URL starts with postgres:// or postgresql://.')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
  });

  it('saves the connection, then empties the fields and shows only the redacted URL', async () => {
    const invoke = fakeBackend();
    render(SettingsView);
    await openPanel();

    await fireEvent.input(await screen.findByLabelText('Connection URL'), { target: { value: POSTGRES_URL } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.getByText('Saved: redacted://db.team.example:5432/mem?sslmode=require')).toBeInTheDocument());
    expect(calls(invoke, 'memory_remote_set')).toEqual([
      { enabled: true, kind: 'postgres', url: POSTGRES_URL, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false },
    ]);
    expect(screen.getByText('Saved. Remote memory is on.')).toBeInTheDocument();

    const url = screen.getByLabelText('Connection URL');
    expect(url).toHaveValue('');
    expect(url).toHaveAttribute('placeholder', 'Leave empty to keep the saved URL');
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(screen.getByRole('switch', { name: 'Remote memory' })).toHaveAttribute('aria-checked', 'true');
    expect(JSON.stringify(localStorage)).not.toContain('secret');
  });

  it('sends a libSQL token with the save and tests with the same input', async () => {
    const invoke = fakeBackend();
    render(SettingsView);
    await openPanel();

    await fireEvent.click(await screen.findByRole('button', { name: 'Database: PostgreSQL' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Remote SQLite (libSQL, Turso)' }));
    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'libsql://team-memory-org.turso.io' } });
    await fireEvent.input(screen.getByLabelText('Auth token'), { target: { value: 'tok-123' } });

    await fireEvent.click(screen.getByRole('button', { name: 'Test connection' }));

    expect(await screen.findByText('Connected to PostgreSQL 16.')).toBeInTheDocument();
    expect(calls(invoke, 'memory_remote_test')).toEqual([{ kind: 'libsql', url: 'libsql://team-memory-org.turso.io', token: 'tok-123' }]);

    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).toMatchObject({ enabled: true, kind: 'libsql', url: 'libsql://team-memory-org.turso.io', token: 'tok-123' });
    await waitFor(() => expect(screen.getByLabelText('Auth token')).toHaveValue(''));
    expect(screen.getByLabelText('Auth token')).toHaveAttribute('placeholder', 'Leave empty to keep the saved token');
  });

  it('keeps the saved URL when the field is empty: tests send null, and changing only a notes choice enables Save', async () => {
    const invoke = fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db.team.example/mem' });
    render(SettingsView);

    const url = await screen.findByLabelText('Connection URL');

    expect(screen.getByRole('switch', { name: 'Remote memory' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByText('Saved: postgres://db.team.example/mem')).toBeInTheDocument();
    expect(url).toHaveAttribute('placeholder', 'Leave empty to keep the saved URL');
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Test connection' })).toBeEnabled();

    await fireEvent.click(screen.getByRole('button', { name: 'Test connection' }));

    await screen.findByText('Connected to PostgreSQL 16.');
    expect(calls(invoke, 'memory_remote_test')).toEqual([{ kind: 'postgres', url: null, token: null }]);

    // Both choices start on the team database.
    const repository = screen.getByRole('radiogroup', { name: 'Repository notes' });
    expect(within(repository).getByRole('radio', { name: 'Team database' })).toBeChecked();
    expect(within(screen.getByRole('radiogroup', { name: 'Global notes' })).getByRole('radio', { name: 'Team database' })).toBeChecked();

    await fireEvent.click(within(repository).getByRole('radio', { name: 'This computer' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')).toEqual([
      { enabled: true, kind: 'postgres', url: null, token: null, repositoryNotes: 'local', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false },
    ]);
  });

  it('keeps scripts on the team database or moves them to this computer, and saves the choice with the rest', async () => {
    const invoke = fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem' });
    render(SettingsView);

    const scripts = await screen.findByRole('radiogroup', { name: 'Scripts' });
    expect(within(scripts).getByRole('radio', { name: 'Team database' })).toBeChecked();
    expect(within(scripts).getByRole('radio', { name: 'This computer' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.click(within(scripts).getByRole('radio', { name: 'This computer' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')).toEqual([
      { enabled: true, kind: 'postgres', url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'local', trustTeamScripts: false },
    ]);
  });

  it('greys the scripts row out and says what to do while the installed Harness Memory keeps no scripts', async () => {
    fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem', scriptsSupported: false });
    render(SettingsView);

    const scripts = await screen.findByRole('radiogroup', { name: 'Scripts' });

    expect(within(scripts).getByRole('radio', { name: 'Team database' })).toBeDisabled();
    expect(within(scripts).getByRole('radio', { name: 'This computer' })).toBeDisabled();
    // The row keeps room for both texts, so the sizing copies are hidden from readers and only one reads aloud.
    expect(screen.getAllByText('Update Harness Memory to choose where scripts are kept.').filter((text) => !text.hasAttribute('aria-hidden'))).toHaveLength(1);
    // The notes rows next to it stay usable.
    expect(within(screen.getByRole('radiogroup', { name: 'Global notes' })).getByRole('radio', { name: 'This computer' })).toBeEnabled();
  });

  it('keeps trusting every team script off until it is switched on, warns what that allows, and saves the choice', async () => {
    const invoke = fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem' });
    render(SettingsView);

    const trust = await screen.findByRole('switch', { name: 'Run team scripts without asking' });
    expect(trust).toHaveAttribute('aria-checked', 'false');
    expect(trust).toBeEnabled();
    expect(screen.getAllByText(/Anyone who can write to the team database can then run code on this computer\./).filter((text) => !text.hasAttribute('aria-hidden'))).toHaveLength(1);
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.click(trust);
    expect(trust).toHaveAttribute('aria-checked', 'true');
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')).toEqual([
      { enabled: true, kind: 'postgres', url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: true },
    ]);
    await waitFor(() => expect(screen.getByRole('switch', { name: 'Run team scripts without asking' })).toHaveAttribute('aria-checked', 'true'));
  });

  it('turns trusting team scripts off when the database address or kind changes, and saves it on only if the user turns it on again for the new one', async () => {
    const invoke = fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem', trustTeamScripts: true });
    render(SettingsView);

    const trust = await screen.findByRole('switch', { name: 'Run team scripts without asking' });
    expect(trust).toHaveAttribute('aria-checked', 'true');

    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'postgres://other.example/mem' } });
    expect(trust).toHaveAttribute('aria-checked', 'false');

    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')[0]).toMatchObject({ url: 'postgres://other.example/mem', trustTeamScripts: false });

    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'postgres://third.example/mem' } });
    await fireEvent.click(trust);
    expect(trust).toHaveAttribute('aria-checked', 'true');
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(2));
    expect(calls(invoke, 'memory_remote_set')[1]).toMatchObject({ url: 'postgres://third.example/mem', trustTeamScripts: true });

    await fireEvent.click(screen.getByRole('button', { name: 'Database: PostgreSQL' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Remote SQLite (libSQL, Turso)' }));
    expect(trust).toHaveAttribute('aria-checked', 'false');
  });

  it('shows the trust switch only while scripts stay in the team database', async () => {
    fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem' });
    render(SettingsView);

    await screen.findByRole('switch', { name: 'Run team scripts without asking' });
    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Scripts' })).getByRole('radio', { name: 'This computer' }));

    await waitFor(() => expect(screen.queryByRole('switch', { name: 'Run team scripts without asking' })).toBeNull());
  });

  it('saves trusting team scripts as off when scripts are moved to this computer, even if it was on before', async () => {
    const invoke = fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem', trustTeamScripts: true });
    render(SettingsView);

    await screen.findByRole('switch', { name: 'Run team scripts without asking' });
    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Scripts' })).getByRole('radio', { name: 'This computer' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')).toEqual([
      { enabled: true, kind: 'postgres', url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'local', trustTeamScripts: false },
    ]);
  });

  it('greys the trust switch out with a hint while the installed Harness Memory cannot honor it', async () => {
    fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem', trustTeamScriptsSupported: false });
    render(SettingsView);

    expect(await screen.findByRole('switch', { name: 'Run team scripts without asking' })).toBeDisabled();
    expect(screen.getAllByText('Update Harness Memory to trust team scripts.').filter((text) => !text.hasAttribute('aria-hidden'))).toHaveLength(1);
  });

  it('shows a failed test in the danger tone with the backend message, and a problem with the saved connection', async () => {
    fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem', problem: 'the saved password was rejected' }, { ok: false, message: 'Connection refused (db.team.example:5432).' });
    render(SettingsView);

    expect(await screen.findByText('Problem with the saved connection: the saved password was rejected')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Test connection' }));

    const message = await screen.findByText('Connection refused (db.team.example:5432).');
    expect(message).toHaveClass('text-danger');
    // Where it is stored and the restart hint live in the (i) tooltip after the Database title.
    await fireEvent.pointerEnter(screen.getByRole('button', { name: 'Database' }).parentElement as HTMLElement);
    expect(await screen.findByText('Stored in /home/me/.config/harness/memory-remote.json, readable only by you.', {}, { timeout: 3000 })).toBeInTheDocument();
  });

  it('asks before turning off saved remote memory, and keeps it on if you cancel', async () => {
    const invoke = fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem' });
    render(SettingsView);
    const remote = await within(memorySection()).findByRole('switch', { name: 'Remote memory' });
    await waitFor(() => expect(remote).toHaveAttribute('aria-checked', 'true'));

    await fireEvent.click(remote);

    expect(await screen.findByText('Turn off remote memory?')).toBeInTheDocument();
    expect(calls(invoke, 'memory_remote_set')).toHaveLength(0);
    expect(remote).toHaveAttribute('aria-checked', 'true');

    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    await waitFor(() => expect(screen.queryByText('Turn off remote memory?')).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_remote_set')).toHaveLength(0);
    expect(remote).toHaveAttribute('aria-checked', 'true');
  });

  it('turns remote memory off after the confirmation and hides the form', async () => {
    const invoke = fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/mem', repositoryNotes: 'local' });
    render(SettingsView);
    const remote = await within(memorySection()).findByRole('switch', { name: 'Remote memory' });
    await waitFor(() => expect(remote).toHaveAttribute('aria-checked', 'true'));

    await fireEvent.click(remote);
    await fireEvent.click(await screen.findByRole('button', { name: 'Turn off' }));

    await waitFor(() => expect(remote).toHaveAttribute('aria-checked', 'false'));
    expect(calls(invoke, 'memory_remote_set')).toEqual([
      { enabled: false, kind: 'postgres', url: null, token: null, repositoryNotes: 'local', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false },
    ]);
    await waitFor(() => expect(screen.queryByLabelText('Connection URL')).not.toBeInTheDocument());
    expect(screen.queryByText('Turn off remote memory?')).not.toBeInTheDocument();
  });

  it('closes an unsaved form without asking', async () => {
    const invoke = fakeBackend();
    render(SettingsView);
    await openPanel();
    await screen.findByLabelText('Connection URL');

    await fireEvent.click(screen.getByRole('switch', { name: 'Remote memory' }));

    await waitFor(() => expect(screen.queryByLabelText('Connection URL')).not.toBeInTheDocument());
    expect(screen.queryByText('Turn off remote memory?')).not.toBeInTheDocument();
    expect(calls(invoke, 'memory_remote_set')).toHaveLength(0);
  });
});
