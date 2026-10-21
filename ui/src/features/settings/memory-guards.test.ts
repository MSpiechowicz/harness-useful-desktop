import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { PICKED_FOLDER } from '$lib/ipc/mock/memory-guards-fixtures';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { PROJECT } from '../../../tests/chat-helpers';
import { startMockApp } from '../../../tests/mock-app';
import MemorySection from './MemorySection.svelte';

/** Every kind resumed, and no repository off. */
async function allowEverything(): Promise<void> {
  await memoryChat.loadGlobal();

  for (const kind of ['notes', 'scripts'] as const) {
    await memoryChat.setGlobalPause(kind, false);

    for (const repo of memoryChat.guards[kind].offRepos) {
      await memoryChat.setRepoOff(kind, repo, false);
    }
  }
}

beforeEach(async () => {
  await startMockApp(PROJECT);
  await memoryRemote.load();
  await allowEverything();
});

afterEach(async () => {
  await allowEverything();
  vi.restoreAllMocks();
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
      expect(within(section).getByText(/^Projects without/)).toBeInTheDocument();
    }

    expect(within(notes).getByText('Every project saves notes.')).toBeInTheDocument();
  });

  it('is not shown when Harness Memory cannot apply save guards', async () => {
    memoryRemote.value = { ...memoryRemote.value!, saveGuardsSupported: false };
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

  it('adds a repository from the folder dialog and lists it, then removes it, without touching the open project', async () => {
    const notes = await card('Notes');
    const open = project.active;
    const recents = project.recents;

    await fireEvent.click(within(notes).getByRole('button', { name: 'Add project…' }));

    expect(await within(notes).findByText(PICKED_FOLDER)).toBeInTheDocument();
    expect(memoryChat.guards.notes.offRepos).toEqual([PICKED_FOLDER]);
    expect(memoryChat.guards.scripts.offRepos).toEqual([]);
    // The dialog is only a picker: the open project and the recent ones stay as they were.
    expect(project.active).toBe(open);
    expect(project.recents).toBe(recents);

    await fireEvent.click(within(notes).getByRole('button', { name: 'Remove website' }));

    // A misclick does not drop the guard: the removal waits for a yes, and Cancel keeps the project on the list.
    const asked = await screen.findByRole('dialog', { name: 'Save notes in website again?' });
    await fireEvent.click(within(asked).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Save notes in website again?' })).not.toBeInTheDocument());
    expect(memoryChat.guards.notes.offRepos).toEqual([PICKED_FOLDER]);

    await fireEvent.click(within(notes).getByRole('button', { name: 'Remove website' }));
    const again = await screen.findByRole('dialog', { name: 'Save notes in website again?' });
    await fireEvent.click(within(again).getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(memoryChat.guards.notes.offRepos).toEqual([]));
    expect(await within(notes).findByText('Every project saves notes.')).toBeInTheDocument();
  });

  it('adds nothing when the folder dialog is cancelled', async () => {
    const notes = await card('Notes');
    // The folder dialog is the next thing the button asks the backend for.
    vi.spyOn(ipc, 'invoke').mockResolvedValueOnce(null);

    await fireEvent.click(within(notes).getByRole('button', { name: 'Add project…' }));

    await waitFor(() => expect(within(notes).getByRole('button', { name: 'Add project…' })).toBeEnabled());
    expect(memoryChat.guards.notes.offRepos).toEqual([]);
  });

  it('keeps the cards greyed out and out of reach while Project memory is off', async () => {
    prefs.value = { ...prefs.value!, memoryEnabled: false };
    const notes = await card('Notes');

    expect(notes.parentElement).toHaveAttribute('inert');
  });
});
