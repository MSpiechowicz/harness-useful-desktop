import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { SETTINGS_SECTIONS } from './sections';
import SettingsView from './SettingsView.svelte';

const SAVED_MONGO_URL = 'mongodb://db.team.example:27017/mem';
const POSTGRES_URL = 'postgres://me:secret@db.team.example:5432/mem?sslmode=require';

/** Changes what the fake backend reports, as if Harness Memory were updated meanwhile. */
let changeBackend: (patch: Partial<MemoryRemote>) => void = () => {};

/** A backend that keeps the team database settings in memory, with Amazon DocumentDB supported or not. */
function fakeBackend(initial: Partial<MemoryRemote> = {}) {
  let state: MemoryRemote = {
    enabled: false,
    kind: null,
    urlRedacted: null,
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
    saveGuardsSupported: false,
    ...initial,
  };
  const passthrough = ipc.invoke.bind(ipc);

  changeBackend = (patch) => {
    state = { ...state, ...patch };
  };

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_remote_get') {
      return state as CommandResult<K>;
    }

    if (name === 'memory_remote_set') {
      const next = args as CommandArgs<'memory_remote_set'>;

      state = { ...state, enabled: next.enabled, kind: next.kind, urlRedacted: next.url === null ? state.urlRedacted : next.url.replace(/\/\/.*@/, '//') };

      return state as CommandResult<K>;
    }

    return passthrough(name, args);
  });
}

function calls(invoke: ReturnType<typeof fakeBackend>, name: string): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);
}

async function openPanel(): Promise<void> {
  const remote = await within(screen.getByRole('region', { name: 'Memory' })).findByRole('switch', { name: 'Remote memory' });
  await waitFor(() => expect(remote).toBeEnabled());
  await fireEvent.click(remote);
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
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('Amazon DocumentDB as the team database', () => {
  it('lists Amazon DocumentDB as the fourth database, with a console-style placeholder and no token field', async () => {
    fakeBackend();
    render(SettingsView);
    await openPanel();

    await fireEvent.click(await screen.findByRole('button', { name: 'Database: PostgreSQL' }));

    expect(screen.getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['PostgreSQL', 'Remote SQLite (libSQL, Turso)', 'MongoDB', 'Amazon DocumentDB']);

    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Amazon DocumentDB' }));

    expect(screen.getByRole('button', { name: 'Database: Amazon DocumentDB' })).toBeInTheDocument();
    expect(screen.getByLabelText('Connection URL')).toHaveAttribute(
      'placeholder',
      'mongodb://user:password@my-cluster.cluster-abc123.eu-central-1.docdb.amazonaws.com:27017/?tls=true&tlsCAFile=global-bundle.pem&replicaSet=rs0&readPreference=secondaryPreferred&retryWrites=false',
    );
    expect(screen.queryByLabelText('Auth token')).not.toBeInTheDocument();
  });

  it('shows the VPN note and the paste hint only while Amazon DocumentDB is selected', async () => {
    fakeBackend();
    render(SettingsView);
    await openPanel();
    const vpn = "AWS only accepts DocumentDB connections from inside the cluster's network. Connect to your VPN before testing or saving.";
    const paste = 'Paste the connection string from the AWS console. The AWS certificate is built in.';

    expect(await screen.findByLabelText('Connection URL')).toBeInTheDocument();
    expect(screen.queryByText(vpn)).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Database: PostgreSQL' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'MongoDB' }));

    expect(screen.queryByText(vpn)).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: /^Database:/ }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Amazon DocumentDB' }));

    expect(await screen.findByText(vpn)).toBeInTheDocument();
    expect(screen.getByText(paste)).toBeInTheDocument();
    expect(screen.queryByText('Your memory needs an update')).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: /^Database:/ }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'PostgreSQL' }));

    await waitFor(() => expect(screen.queryByText(vpn)).not.toBeInTheDocument());
  });

  it('accepts the mongodb schemes for Amazon DocumentDB and saves the pasted console string as documentdb', async () => {
    const invoke = fakeBackend();
    render(SettingsView);
    await openPanel();
    await fireEvent.click(await screen.findByRole('button', { name: 'Database: PostgreSQL' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Amazon DocumentDB' }));
    const url = screen.getByLabelText('Connection URL');
    const docdb = 'mongodb://me:secret@my-cluster.cluster-abc.eu-central-1.docdb.amazonaws.com:27017/mem?tls=true&replicaSet=rs0&tlsCAFile=global-bundle.pem';

    await fireEvent.input(url, { target: { value: POSTGRES_URL } });
    await fireEvent.blur(url);

    expect(await screen.findByText('An Amazon DocumentDB URL starts with mongodb:// or mongodb+srv://.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.input(url, { target: { value: docdb } });

    expect(screen.getByRole('button', { name: 'Test connection' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();

    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
    expect(calls(invoke, 'memory_remote_set')).toEqual([expect.objectContaining({ enabled: true, kind: 'documentdb', url: docdb, token: null })]);
  });

  it('keeps Amazon DocumentDB listed but cannot test or save it while the installed Harness Memory is too old', async () => {
    const invoke = fakeBackend({ documentdbSupported: false });
    render(SettingsView);
    await openPanel();
    await fireEvent.click(await screen.findByRole('button', { name: 'Database: PostgreSQL' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Amazon DocumentDB' }));

    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'mongodb://me:secret@c.docdb.amazonaws.com:27017/mem' } });

    expect(await screen.findByText('Your memory needs an update')).toBeInTheDocument();
    // The update is offered right here, not on another page.
    expect(screen.getByRole('button', { name: 'Update Harness Memory' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Test connection' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.click(screen.getByRole('button', { name: /^Database:/ }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'MongoDB' }));
    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'mongodb://host/db' } });

    expect(screen.queryByText('Your memory needs an update')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Test connection' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
    expect(calls(invoke, 'memory_remote_set')).toEqual([]);
  });

  it.each(['en', 'de', 'es', 'fr', 'pl'] as const)('lists documentdb, aws and amazon among the Memory search words in %s', async (language) => {
    await i18n.load(language);

    const memory = SETTINGS_SECTIONS.find((section) => section.id === 'memory');

    expect(memory?.keywords.split(' ')).toEqual(expect.arrayContaining(['documentdb', 'aws', 'amazon']));

    await i18n.load('en');
  });

  describe('with a saved URL', () => {
    async function openSaved(name: string): Promise<void> {
      render(SettingsView);
      await screen.findByRole('button', { name });
    }

    async function pick(from: string, to: string): Promise<void> {
      await fireEvent.click(screen.getByRole('button', { name: `Database: ${from}` }));
      await fireEvent.click(screen.getByRole('menuitemradio', { name: to }));
    }

    const mongo: Partial<MemoryRemote> = { enabled: true, kind: 'mongodb', urlRedacted: SAVED_MONGO_URL };

    it('keeps the saved URL when switching between MongoDB and Amazon DocumentDB, and sends no URL', async () => {
      const invoke = fakeBackend(mongo);
      await openSaved('Database: MongoDB');

      await pick('MongoDB', 'Amazon DocumentDB');

      expect(screen.getByText(`Saved: ${SAVED_MONGO_URL}`, { exact: false })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Test connection' })).toBeEnabled();
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();

      await fireEvent.click(screen.getByRole('button', { name: 'Test connection' }));
      await waitFor(() => expect(calls(invoke, 'memory_remote_test')).toHaveLength(1));
      expect(calls(invoke, 'memory_remote_test')).toEqual([{ kind: 'documentdb', url: null, token: null }]);

      await waitFor(() => expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled());
      await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() => expect(calls(invoke, 'memory_remote_set')).toHaveLength(1));
      expect(calls(invoke, 'memory_remote_set')).toEqual([expect.objectContaining({ enabled: true, kind: 'documentdb', url: null })]);

      await waitFor(() => expect(screen.getByRole('button', { name: 'Database: Amazon DocumentDB' })).toBeInTheDocument());
    });

    it('switches back from Amazon DocumentDB to MongoDB the same way', async () => {
      fakeBackend({ ...mongo, kind: 'documentdb' });
      await openSaved('Database: Amazon DocumentDB');

      await pick('Amazon DocumentDB', 'MongoDB');

      expect(screen.getByRole('button', { name: 'Test connection' })).toBeEnabled();
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
    });

    it('still asks for a URL when the saved one belongs to another kind of database', async () => {
      fakeBackend({ enabled: true, kind: 'postgres', urlRedacted: 'postgres://db.team.example:5432/mem' });
      await openSaved('Database: PostgreSQL');

      await pick('PostgreSQL', 'Amazon DocumentDB');

      expect(screen.queryByText('Saved:', { exact: false })).not.toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Test connection' })).toBeDisabled();
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

      await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: SAVED_MONGO_URL } });

      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
    });
  });

  describe('while the installed Harness Memory is too old', () => {
    const old: Partial<MemoryRemote> = { enabled: true, kind: 'mongodb', urlRedacted: SAVED_MONGO_URL, documentdbSupported: false };

    /** Lets the promises of a fake-timer step settle; the testing-library waits would hang on faked timers. */
    async function advance(ms: number): Promise<void> {
      await vi.advanceTimersByTimeAsync(ms);
    }

    it('looks again by itself and unlocks Test and Save once Harness Memory is updated, keeping the typed URL', async () => {
      const invoke = fakeBackend(old);
      render(SettingsView);
      await screen.findByRole('button', { name: 'Database: MongoDB' });
      vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });

      await fireEvent.click(screen.getByRole('button', { name: 'Database: MongoDB' }));
      await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Amazon DocumentDB' }));
      const typed = 'mongodb://me:secret@c.docdb.amazonaws.com:27017/mem';
      await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: typed } });
      const before = calls(invoke, 'memory_remote_get').length;

      expect(screen.getByText('Your memory needs an update')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

      changeBackend({ documentdbSupported: true });
      await advance(10_000);

      expect(calls(invoke, 'memory_remote_get').length).toBeGreaterThan(before);
      expect(screen.queryByText('Your memory needs an update')).not.toBeInTheDocument();
      expect(screen.getByLabelText('Connection URL')).toHaveValue(typed);
      expect(screen.getByRole('button', { name: 'Test connection' })).toBeEnabled();
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();

      const settled = calls(invoke, 'memory_remote_get').length;

      await advance(60_000);

      expect(calls(invoke, 'memory_remote_get')).toHaveLength(settled);
    });

    it('stops polling when another database is chosen', async () => {
      const invoke = fakeBackend(old);
      render(SettingsView);
      await screen.findByRole('button', { name: 'Database: MongoDB' });
      vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });

      await fireEvent.click(screen.getByRole('button', { name: 'Database: MongoDB' }));
      await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Amazon DocumentDB' }));
      await advance(0);
      const polling = calls(invoke, 'memory_remote_get').length;

      await advance(10_000);

      expect(calls(invoke, 'memory_remote_get').length).toBeGreaterThan(polling);

      await fireEvent.click(screen.getByRole('button', { name: 'Database: Amazon DocumentDB' }));
      await fireEvent.click(screen.getByRole('menuitemradio', { name: 'MongoDB' }));
      await advance(0);
      const stopped = calls(invoke, 'memory_remote_get').length;

      await advance(60_000);

      expect(calls(invoke, 'memory_remote_get')).toHaveLength(stopped);
    });
  });
});
