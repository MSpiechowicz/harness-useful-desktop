import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { SessionState } from '$lib/ipc/bindings/SessionState';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { defaultGuards } from '$lib/stores/memory-chat';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryHeld } from '$lib/stores/memoryHeld.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { project } from '$lib/stores/project.svelte';

import { chatSession, PROJECT } from '../../../tests/chat-helpers';
import { startMockApp } from '../../../tests/mock-app';
import MemoryButton from './MemoryButton.svelte';

const KEY = '11111111-2222-4333-8444-555555555555';
const ROOT = PROJECT;
const NOT_A_REPOSITORY = fixtures.recentProjects[1].path;
/** The chats the test gave an identity, released afterwards: the store outlives the test. */
const attached: string[] = [];

async function show(folder = PROJECT) {
  const { session } = chatSession([], { project: folder, memoryGuards: true });
  memoryChat.attach(session.key, { key: KEY, guards: defaultGuards({ notes: false, scripts: false }) });
  attached.push(session.key);
  render(MemoryButton, { session });

  return session;
}

async function open(): Promise<HTMLElement> {
  await fireEvent.click(await screen.findByRole('button', { name: /^Memory/ }));

  return screen.findByRole('dialog', { name: 'Saving to memory' });
}

beforeEach(async () => {
  await startMockApp(PROJECT);
  harness.summaries = fixtures.harnesses;
  await memoryRemote.load();
  await memoryChat.loadGlobal();
});

afterEach(() => {
  attached.splice(0).forEach((key) => memoryChat.release(key));
  memoryHeld.items = [];
  vi.restoreAllMocks();
});

describe('the Memory button of a chat', () => {
  it('is in the top bar with the number of saves waiting for the user', async () => {
    memoryHeld.items = sampleHeld(KEY);

    await show();

    const button = await screen.findByRole('button', { name: /^Memory/ });
    expect(button).toHaveTextContent('Memory');
    expect(button).toHaveTextContent('4');
  });

  it('is not shown when Harness Memory cannot apply save guards', async () => {
    memoryRemote.value = { ...memoryRemote.value!, saveGuardsSupported: false };

    await show();

    expect(screen.queryByRole('button', { name: /^Memory/ })).toBeNull();
  });

  it('is not shown when the harness does not pass the chat’s key to Harness Memory', async () => {
    harness.summaries = fixtures.harnesses.map((summary) => ({ ...summary, capabilities: { ...summary.capabilities, memoryChatGuards: false } }));

    await show();

    expect(screen.queryByRole('button', { name: /^Memory/ })).toBeNull();
  });

  it('is not shown when the chat’s key reached no harness-memory server', async () => {
    const { session } = chatSession([], { harness: 'codex', project: PROJECT, memoryGuards: false });
    memoryChat.attach(session.key, { key: KEY, guards: defaultGuards({ notes: false, scripts: false }) });
    attached.push(session.key);
    render(MemoryButton, { session });

    expect(screen.queryByRole('button', { name: /^Memory/ })).toBeNull();
  });

  it('follows the chat when a new thread finds a harness-memory server, or none any more', async () => {
    const { session } = chatSession([], { harness: 'codex', project: PROJECT, memoryGuards: false });
    memoryChat.attach(session.key, { key: KEY, guards: defaultGuards({ notes: false, scripts: false }) });
    attached.push(session.key);
    render(MemoryButton, { session });
    let seq = 0;
    const state = (memoryGuards: boolean) => {
      const sent: SessionState = { sessionId: 'thread', sessionName: null, model: null, thinkingLevel: null, agent: null, isStreaming: false, isCompacting: false, messageCount: 0, queuedCount: 0, contextUsage: null, memoryGuards };
      session.ingest({ sessionKey: session.key, seq: ++seq, event: { type: 'state', state: sent } });
      session.flush();
    };

    state(true);
    expect(await screen.findByRole('button', { name: /^Memory/ })).toBeInTheDocument();

    state(false);
    await waitFor(() => expect(screen.queryByRole('button', { name: /^Memory/ })).toBeNull());
  });

  it('is not shown for a chat that has no memory identity', async () => {
    const { session } = chatSession([], { project: PROJECT });
    render(MemoryButton, { session });

    expect(screen.queryByRole('button', { name: /^Memory/ })).toBeNull();
  });
});

describe('the Memory popover', () => {
  it('has a card each for notes and scripts, with this chat, this repository and ask before saving', async () => {
    await show();
    const panel = await open();

    expect(within(panel).getByText('Reading stays on')).toBeInTheDocument();
    for (const kind of ['Notes', 'Scripts']) {
      const card = within(panel).getByRole('region', { name: kind });
      expect(within(card).getByRole('switch', { name: `${kind}: This chat` })).toBeChecked();
      expect(within(card).getByRole('switch', { name: `${kind}: This project` })).toBeChecked();
      expect(within(card).getByRole('switch', { name: `${kind}: Ask before saving` })).not.toBeChecked();
    }
  });

  it('turns saving off for the chat only, and dims what no longer applies', async () => {
    const session = await show();
    const panel = await open();

    await fireEvent.click(within(panel).getByRole('switch', { name: 'Notes: This chat' }));

    expect(memoryChat.get(session.key)?.guards.notes.on).toBe(false);
    expect(memoryChat.get(session.key)?.guards.scripts.on).toBe(true);
    await waitFor(() => expect(within(panel).getByRole('switch', { name: 'Notes: Ask before saving' })).toBeDisabled());
    expect(within(panel).getByRole('switch', { name: 'Scripts: Ask before saving' })).toBeEnabled();
  });

  it('turns saving off for the whole repository, for every chat in it, and says so in Settings terms', async () => {
    const session = await show();
    const panel = await open();

    await fireEvent.click(within(panel).getByRole('switch', { name: 'Scripts: This project' }));

    await waitFor(() => expect(memoryChat.guards.scripts.offRepos).toEqual([ROOT]));
    expect(memoryChat.guards.notes.offRepos).toEqual([]);
    expect(memoryChat.get(session.key)?.guards.scripts.on).toBe(true);
    expect(within(panel).getByText('Off while demo is off')).toBeInTheDocument();
    expect(within(panel).getByRole('switch', { name: 'Scripts: This chat' })).toBeDisabled();

    await fireEvent.click(within(panel).getByRole('switch', { name: 'Scripts: This project' }));

    await waitFor(() => expect(memoryChat.guards.scripts.offRepos).toEqual([]));
  });

  it('offers the repository switch for a folder that is no git repository, which then stands for the folder itself', async () => {
    expect(fixtures.recentProjects[1].isGitRepo).toBe(false);
    await show(NOT_A_REPOSITORY);
    const panel = await open();

    const repository = within(panel).getByRole('switch', { name: 'Notes: This project' });
    expect(repository).toBeChecked();

    await fireEvent.click(repository);

    await waitFor(() => expect(memoryChat.guards.notes.offRepos).toEqual([NOT_A_REPOSITORY]));
    expect(within(panel).getByRole('switch', { name: 'Notes: This project' })).not.toBeChecked();
  });

  it('leaves out the repository switches for a folder whose root is not known', async () => {
    await show('/nowhere/unknown');
    const panel = await open();

    expect(within(panel).queryByRole('switch', { name: 'Notes: This project' })).toBeNull();
    expect(within(panel).getByRole('switch', { name: 'Notes: This chat' })).toBeInTheDocument();
  });

  it('pauses both kinds with one Pause, which then offers Resume and shows in the button', async () => {
    const session = await show();
    const panel = await open();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Pause' }));

    await waitFor(() => expect(memoryChat.get(session.key)?.guards.paused).toBe(true));
    expect(screen.getByRole('button', { name: /^Memory paused/ })).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Check again' })).toBeDisabled();
    expect(within(panel).queryByRole('button', { name: 'Pause' })).toBeNull();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Resume' }));

    await waitFor(() => expect(memoryChat.get(session.key)?.guards.paused).toBe(false));
  });

  it('asks the agent to check again, and cannot when both kinds are off', async () => {
    const session = await show();
    const check = vi.spyOn(memoryChat, 'checkAgain').mockResolvedValue(true);
    const panel = await open();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Check again' }));
    expect(check).toHaveBeenCalledWith(session.key);

    await fireEvent.click(within(panel).getByRole('switch', { name: 'Notes: This chat' }));
    await fireEvent.click(within(panel).getByRole('switch', { name: 'Scripts: This chat' }));

    await waitFor(() => expect(within(panel).getByRole('button', { name: 'Check again' })).toBeDisabled());
    expect(within(panel).getByRole('button', { name: 'Pause' })).toBeDisabled();
    expect(screen.getByRole('button', { name: /^Memory off/ })).toBeInTheDocument();
  });

  it('brings the waiting saves into view with Review', async () => {
    memoryHeld.items = sampleHeld(KEY);
    const session = await show();
    const panel = await open();

    expect(within(panel).getByText('4 waiting for you')).toBeInTheDocument();
    await fireEvent.click(within(panel).getByRole('button', { name: 'Review' }));

    expect(memoryChat.reviewRequests(session.key)).toBe(1);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });
});
