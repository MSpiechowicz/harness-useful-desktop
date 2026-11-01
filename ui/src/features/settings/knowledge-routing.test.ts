import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

const CHANGE = 'Change where documents are kept?';

/** A server with a remote database that keeps the routing in memory, the way the real config file does. */
function fakeBackend(initial: Partial<KnowledgeRemote> = {}) {
  let state = {
    enabled: true,
    kind: 'postgres',
    urlRedacted: 'postgres://db/knowledge',
    hasToken: false,
    embedderSupported: true,
    repositoryDocuments: 'remote',
    globalDocuments: 'remote',
    repositoryDocumentsEnv: false,
    globalDocumentsEnv: false,
    routingSupported: true,
    configPath: '~/.config/harness-knowledge/config.toml',
    problem: null,
    ...initial,
  } as KnowledgeRemote;
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'knowledge_remote_get') {
      return state as CommandResult<K>;
    }

    if (name === 'knowledge_remote_set') {
      const next = args as CommandArgs<'knowledge_remote_set'>;

      state = { ...state, repositoryDocuments: next.repositoryDocuments ?? state.repositoryDocuments, globalDocuments: next.globalDocuments ?? state.globalDocuments };

      return state as CommandResult<K>;
    }

    return passthrough(name, args);
  });
}

function saves(invoke: ReturnType<typeof fakeBackend>): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === 'knowledge_remote_set').map(([, args]) => args);
}

async function choose(group: 'Project documents' | 'Global documents', place: string): Promise<void> {
  await fireEvent.click(within(await screen.findByRole('radiogroup', { name: group })).getByRole('radio', { name: place }));
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
  knowledgeRemote.value = null;
  knowledgeChanges.reset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Kept in for documents', () => {
  it('has a row for project documents and one for global documents with the three places of memory, and a hint for the chosen one', async () => {
    fakeBackend();
    render(SettingsView);

    for (const group of ['Project documents', 'Global documents'] as const) {
      const radios = within(await screen.findByRole('radiogroup', { name: group })).getAllByRole('radio');

      expect(radios.map((radio) => radio.textContent?.trim())).toEqual(['Remote', 'This computer', 'Remote + this computer']);
      expect(radios.every((radio) => (radio as HTMLButtonElement).disabled === false)).toBe(true);
    }

    expect(screen.getAllByText('Read and saved in the remote database.').length).toBeGreaterThan(0);
    expect(screen.queryByText('Update Harness Knowledge to choose where documents are kept.')).not.toBeInTheDocument();
  });

  it('greys out the other places with a note while the installed server cannot route documents', async () => {
    fakeBackend({ routingSupported: false });
    render(SettingsView);

    const group = await screen.findByRole('radiogroup', { name: 'Project documents' });

    expect(within(group).getByRole('radio', { name: 'This computer' })).toBeDisabled();
    expect(within(group).getByRole('radio', { name: 'Remote + this computer' })).toBeDisabled();
    expect(within(group).getByRole('radio', { name: 'Remote' })).toBeEnabled();
    expect(screen.getByText('Update Harness Knowledge to choose where documents are kept.')).toBeInTheDocument();
  });

  it('says what a change does before it is saved, and saves only once the user agrees', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await choose('Project documents', 'Remote + this computer');
    await saveForm();

    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getByText('Project documents')).toBeInTheDocument();
    expect(within(dialog).getByText('Nothing is hidden. You choose where each import is saved.')).toBeInTheDocument();
    expect(saves(invoke)).toHaveLength(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).toMatchObject({ repositoryDocuments: 'remoteLocal', globalDocuments: 'remote' });
  });

  it('warns that remote documents are hidden when a kind moves to this computer, and saves nothing on cancel', async () => {
    const invoke = fakeBackend();
    render(SettingsView);

    await choose('Global documents', 'This computer');
    await saveForm();

    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getByText('Remote documents are hidden on this computer. They stay in the remote database.')).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));

    expect(saves(invoke)).toHaveLength(0);
  });

  it('does not send the routing to a server that cannot take it', async () => {
    const invoke = fakeBackend({ routingSupported: false });
    render(SettingsView);

    await screen.findByRole('radiogroup', { name: 'Project documents' });
    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'postgres://me:pw@db.ok.example/knowledge' } });
    await saveForm();

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(saves(invoke)[0]).not.toHaveProperty('repositoryDocuments');
  });
});

describe('Kept in on a first connection', () => {
  async function connect(): Promise<void> {
    const toggle = await screen.findByRole('switch', { name: 'Remote knowledge' });
    await waitFor(() => expect(toggle).toBeEnabled());
    await fireEvent.click(toggle);
    await fireEvent.input(await screen.findByLabelText('Connection URL'), { target: { value: 'postgres://me:pw@db.ok.example/knowledge' } });
    await saveForm();
  }

  it('warns that the documents on this computer are hidden when the remote database will hold the documents instead', async () => {
    const invoke = fakeBackend({ enabled: false, kind: null, urlRedacted: null });
    knowledgeChanges.pulse = { documents: 3, generation: 1, held: 0, offline: false };
    render(SettingsView);

    await connect();

    const dialog = await screen.findByRole('dialog', { name: CHANGE });
    expect(within(dialog).getAllByText('Documents saved on this computer are hidden. They stay on this computer.')).toHaveLength(2);
    expect(saves(invoke)).toHaveLength(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
  });

  it('warns too while how many documents this computer holds is not known yet', async () => {
    const invoke = fakeBackend({ enabled: false, kind: null, urlRedacted: null });
    knowledgeChanges.pulse = null;
    render(SettingsView);

    await connect();

    expect(await screen.findByRole('dialog', { name: CHANGE })).toBeInTheDocument();
    expect(saves(invoke)).toHaveLength(0);
  });

  it('says nothing while this computer holds no documents', async () => {
    const invoke = fakeBackend({ enabled: false, kind: null, urlRedacted: null });
    knowledgeChanges.pulse = { documents: 0, generation: 1, held: 0, offline: false };
    render(SettingsView);

    await connect();

    await waitFor(() => expect(saves(invoke)).toHaveLength(1));
    expect(screen.queryByRole('dialog', { name: CHANGE })).toBeNull();
  });
});

describe('Kept in when the saved routing changes', () => {
  it('shows the routing the backend now reports instead of the one the form started with', async () => {
    fakeBackend();
    render(SettingsView);

    const group = await screen.findByRole('radiogroup', { name: 'Project documents' });
    expect(within(group).getByRole('radio', { name: 'Remote' })).toHaveAttribute('aria-checked', 'true');

    knowledgeRemote.value = { ...(knowledgeRemote.value as KnowledgeRemote), repositoryDocuments: 'local' };

    await waitFor(() => expect(within(group).getByRole('radio', { name: 'This computer' })).toHaveAttribute('aria-checked', 'true'));
  });
});

describe('Kept in when an environment variable sets the routing', () => {
  it('locks the row it sets, shows its value and says why, leaving the other row free', async () => {
    fakeBackend({ repositoryDocuments: 'local', repositoryDocumentsEnv: true });
    render(SettingsView);

    const project = await screen.findByRole('radiogroup', { name: 'Project documents' });
    const global = screen.getByRole('radiogroup', { name: 'Global documents' });

    expect(within(project).getByRole('radio', { name: 'This computer' })).toHaveAttribute('aria-checked', 'true');
    expect(within(project).getAllByRole('radio').every((radio) => (radio as HTMLButtonElement).disabled)).toBe(true);
    expect(within(global).getAllByRole('radio').every((radio) => (radio as HTMLButtonElement).disabled === false)).toBe(true);
    expect(screen.getAllByText('Set by an environment variable')).toHaveLength(1);
  });
});
