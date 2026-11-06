import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import type { EcosystemRelease } from '$lib/ipc/bindings/EcosystemRelease';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { PICKED_FOLDER } from '$lib/ipc/mock/memory-guards-fixtures';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { PROJECT } from '../../../tests/chat-helpers';
import { startMockApp } from '../../../tests/mock-app';
import MemorySection from './MemorySection.svelte';

/** Every kind resumed, and saving on in no project: where a user who turned nothing on starts. */
async function resetGuards(): Promise<void> {
  await memoryChat.loadGlobal();

  for (const kind of ['notes', 'scripts'] as const) {
    await memoryChat.setGlobalPause(kind, false);
    await memoryChat.setEveryProject(kind, false);

    for (const repo of memoryChat.guards[kind].onRepos) {
      await memoryChat.setRepoOn(kind, repo, false);
    }
  }
}

beforeEach(async () => {
  await startMockApp(PROJECT);
  await memoryRemote.load();
  await resetGuards();
});

afterEach(async () => {
  vi.restoreAllMocks();
  await resetGuards();
  ecosystem.reset();
});

/** The Notes or Scripts card, found by its heading. */
async function card(name: 'Notes' | 'Scripts'): Promise<HTMLElement> {
  render(MemorySection, { id: 'memory' });

  return (await screen.findByRole('heading', { name })).closest('section') as HTMLElement;
}

describe('Settings, Memory: what agents may save', () => {
  it('has a Notes and a Scripts card under Project memory', async () => {
    const notes = await card('Notes');
    const scripts = screen.getByRole('heading', { name: 'Scripts' }).closest('section') as HTMLElement;

    for (const section of [notes, scripts]) {
      expect(within(section).getByText('Pause saving')).toBeInTheDocument();
      expect(within(section).getByText('Ask before saving')).toBeInTheDocument();
      expect(within(section).getByText('Save in every project')).toBeInTheDocument();
      expect(within(section).getByText(/^Projects with (notes|scripts)$/)).toBeInTheDocument();
    }

    expect(within(notes).getByText('No project saves notes yet.')).toBeInTheDocument();
  });

  it('is not shown when Harness Memory cannot apply save guards', async () => {
    memoryRemote.value = { ...memoryRemote.value!, saveGuardsSupported: false, saveOptInSupported: false };
    render(MemorySection, { id: 'memory' });

    expect(await screen.findByText('Project memory')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Notes' })).toBeNull();
    expect(screen.queryByRole('heading', { name: 'Scripts' })).toBeNull();
  });

  it('pauses one kind in every chat and says so above its card, with Resume there', async () => {
    const notes = await card('Notes');

    await fireEvent.click(within(notes).getByRole('switch', { name: 'Notes: Pause saving' }));

    await waitFor(() => expect(memoryChat.guards.notes.paused).toBe(true));
    expect(memoryChat.guards.scripts.paused).toBe(false);
    expect(await within(notes).findByText('Saving notes is paused in every chat.')).toBeInTheDocument();

    await fireEvent.click(within(notes).getByRole('button', { name: 'Resume' }));

    await waitFor(() => expect(memoryChat.guards.notes.paused).toBe(false));
  });

  it('chooses what new chats start with for "Ask before saving", separately for notes and scripts', async () => {
    const scripts = (await card('Notes'), screen.getByRole('heading', { name: 'Scripts' }).closest('section') as HTMLElement);

    await fireEvent.click(within(scripts).getByRole('switch', { name: 'Scripts: Ask before saving' }));

    await waitFor(() => expect(prefs.value?.memoryScriptsAskDefault).toBe(true));
    expect(prefs.value?.memoryNotesAskDefault).toBe(false);
  });

  it('adds a project from the folder dialog and lists it, then removes it without asking, and without touching the open project', async () => {
    const notes = await card('Notes');
    const open = project.active;
    const recents = project.recents;

    await fireEvent.click(within(notes).getByRole('button', { name: 'Add project…' }));

    expect(await within(notes).findByText(PICKED_FOLDER)).toBeInTheDocument();
    expect(memoryChat.guards.notes.onRepos).toEqual([PICKED_FOLDER]);
    expect(memoryChat.guards.scripts.onRepos).toEqual([]);
    // The dialog is only a picker: the open project and the recent ones stay as they were.
    expect(project.active).toBe(open);
    expect(project.recents).toBe(recents);

    // Removing only turns saving off again, so there is nothing to confirm.
    await fireEvent.click(within(notes).getByRole('button', { name: 'Remove website' }));

    expect(screen.queryByRole('dialog')).toBeNull();
    await waitFor(() => expect(memoryChat.guards.notes.onRepos).toEqual([]));
    expect(await within(notes).findByText('No project saves notes yet.')).toBeInTheDocument();
  });

  it('says when a listed project no longer turns anything on, and still removes it', async () => {
    const notes = await card('Notes');

    await memoryChat.addRepo('notes', PICKED_FOLDER);
    memoryChat.global = { ...memoryChat.guards, notes: { ...memoryChat.guards.notes, deadRepos: [PICKED_FOLDER] } };

    expect(await within(notes).findByText(/Folder not found, or it is now a link/)).toBeInTheDocument();

    memoryChat.global = { ...memoryChat.guards, notes: { ...memoryChat.guards.notes, everyProject: true } };

    expect(await within(notes).findByText(/this entry does nothing once that is off/)).toBeInTheDocument();

    memoryChat.global = { ...memoryChat.guards, notes: { ...memoryChat.guards.notes, everyProject: false, deadRepos: [] } };

    await waitFor(() => expect(within(notes).queryByText(/Folder not found, or it is now a link/)).toBeNull());
  });

  it('says when the remote of a listed project changed, and turns it on again from the row', async () => {
    const notes = await card('Notes');

    await memoryChat.addRepo('notes', PICKED_FOLDER);
    memoryChat.global = { ...memoryChat.guards, notes: { ...memoryChat.guards.notes, changedRepos: [PICKED_FOLDER] } };

    expect(await within(notes).findByText(/The remote of this project changed/)).toBeInTheDocument();

    await fireEvent.click(within(notes).getByRole('button', { name: /^Turn .* on again$/ }));

    await waitFor(() => expect(within(notes).queryByText(/The remote of this project changed/)).toBeNull());
    expect(memoryChat.guards.notes.onRepos).toEqual([PICKED_FOLDER]);
  });

  it('turns saving on for every project only after a yes, and off again at once', async () => {
    const scripts = (await card('Notes'), screen.getByRole('heading', { name: 'Scripts' }).closest('section') as HTMLElement);
    const every = within(scripts).getByRole('switch', { name: 'Scripts: Save in every project' });
    expect(every).not.toBeChecked();

    await fireEvent.click(every);

    // The question comes first, and Cancel leaves everything as it was.
    const asked = await screen.findByRole('dialog', { name: 'Save scripts in every project?' });
    expect(within(asked).getByText('Agents can save scripts in every project, including ones you open later.')).toBeInTheDocument();
    await fireEvent.click(within(asked).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(memoryChat.guards.scripts.everyProject).toBe(false);
    expect(every).not.toBeChecked();

    await fireEvent.click(every);
    await fireEvent.click(within(await screen.findByRole('dialog', { name: 'Save scripts in every project?' })).getByRole('button', { name: 'Turn on' }));

    await waitFor(() => expect(memoryChat.guards.scripts.everyProject).toBe(true));
    expect(memoryChat.guards.notes.everyProject).toBe(false);
    expect(every).toBeChecked();
    expect(within(scripts).getAllByText(/^On\. Agents save scripts in every project\./).filter((text) => !text.hasAttribute('aria-hidden'))).toHaveLength(1);

    await fireEvent.click(every);

    expect(screen.queryByRole('dialog')).toBeNull();
    await waitFor(() => expect(memoryChat.guards.scripts.everyProject).toBe(false));
  });

  it('keeps the list of projects available while saving is on for every project, so it can be prepared', async () => {
    await memoryChat.setEveryProject('notes', true);
    await memoryChat.setRepoOn('notes', PICKED_FOLDER, true);
    const notes = await card('Notes');

    expect(await within(notes).findByText(PICKED_FOLDER)).toBeInTheDocument();
    expect(within(notes).getByRole('button', { name: 'Add project…' })).toBeEnabled();
    expect(within(notes).getByRole('button', { name: 'Remove website' })).toBeEnabled();
  });

  it('adds nothing when the folder dialog is cancelled', async () => {
    const notes = await card('Notes');
    // The folder dialog is the next thing the button asks the backend for.
    vi.spyOn(ipc, 'invoke').mockResolvedValueOnce(null);

    await fireEvent.click(within(notes).getByRole('button', { name: 'Add project…' }));

    await waitFor(() => expect(within(notes).getByRole('button', { name: 'Add project…' })).toBeEnabled());
    expect(memoryChat.guards.notes.onRepos).toEqual([]);
  });

  it('keeps the cards greyed out and out of reach while Project memory is off', async () => {
    prefs.value = { ...prefs.value!, memoryEnabled: false };
    const notes = await card('Notes');

    expect(notes.parentElement).toHaveAttribute('inert');
  });
});

describe('Settings, Memory: a Harness Memory that predates choosing where to save', () => {
  /** Harness Memory is installed (the mock's ecosystem reading says it is not until an install), a newer release is out, and the server predates the opt-in until the install ran. */
  function updatable(installOk = true) {
    const real = ipc.invoke.bind(ipc);
    const result: DashboardInstallResult = { ok: installOk, summary: installOk ? 'Installed.' : 'error', exitCode: 0, timedOut: false, truncated: false, version: '9.9.9', command: 'install.sh' };
    let updated = false;

    return {
      spy: vi.spyOn(ipc, 'invoke').mockImplementation((async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
        if (name === 'ecosystem_status') {
          const status = (await real(name, args)) as CommandResult<'ecosystem_status'>;

          return { ...status, items: status.items.map((item) => (item.id === 'memory' ? { ...item, installed: true, version: '1.0.0' } : item)) } as CommandResult<K>;
        }

        if (name === 'ecosystem_releases') {
          const others = ((await real(name, args)) as EcosystemRelease[]).filter((entry) => entry.id !== 'memory');

          return [...others, { id: 'memory', latestVersion: '9.9.9', updateAvailable: true }] as CommandResult<K>;
        }

        if (name === 'setup_install_memory') {
          updated = installOk;

          return result as CommandResult<K>;
        }

        if (name === 'memory_remote_get') {
          const remote = (await real(name, args)) as CommandResult<'memory_remote_get'>;

          return { ...remote, saveOptInSupported: updated } as CommandResult<K>;
        }

        return real(name, args);
      }) as typeof ipc.invoke),
    };
  }

  it('offers the update right in the Notes group, with no switches until it is done', async () => {
    updatable();
    await memoryRemote.load();
    render(MemorySection, { id: 'memory' });

    expect(await screen.findByText('Update Harness Memory to choose where notes and scripts are saved.')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Notes' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Update Harness Memory' }));
    const dialog = await screen.findByRole('dialog');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Update Harness Memory' }));

    // The server answers as an updated one now: the row goes away and the cards come.
    expect(await screen.findByRole('heading', { name: 'Notes' })).toBeInTheDocument();
    expect(screen.queryByText('Update Harness Memory to choose where notes and scripts are saved.')).toBeNull();
  });

  it('says nothing when Harness Memory is not installed', async () => {
    const real = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation((async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
      if (name === 'ecosystem_status') {
        const status = (await real(name, args)) as CommandResult<'ecosystem_status'>;

        return { ...status, items: status.items.map((item) => (item.id === 'memory' ? { ...item, installed: false, version: null } : item)) } as CommandResult<K>;
      }

      if (name === 'memory_remote_get') {
        const remote = (await real(name, args)) as CommandResult<'memory_remote_get'>;

        return { ...remote, saveGuardsSupported: false, saveOptInSupported: false } as CommandResult<K>;
      }

      return real(name, args);
    }) as typeof ipc.invoke);
    await memoryRemote.load();
    await ecosystem.load(true);
    render(MemorySection, { id: 'memory' });

    expect(await screen.findByText('Project memory')).toBeInTheDocument();
    expect(screen.queryByText('Update Harness Memory to choose where notes and scripts are saved.')).toBeNull();
  });
});
