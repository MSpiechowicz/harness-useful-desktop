import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { SessionState } from '$lib/ipc/bindings/SessionState';
import { ipc } from '$lib/ipc/client';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { defaultGuards, GUARD_KINDS } from '$lib/stores/memory-chat';
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
/** A git worktree of the project: its saving root is the project itself. */
const WORKTREE = '/home/demo/worktrees/feature';
/** A folder the backend no longer finds, so it has no saving root. */
const GONE = '/nowhere/unknown';
/** The chats the test gave an identity, released afterwards: the store outlives the test. */
const attached: string[] = [];

async function show(folder = PROJECT) {
  const { session } = chatSession([], { project: folder, memoryGuards: true });
  memoryChat.attach(session.key, { key: KEY, guards: defaultGuards({ notes: false, scripts: false }) });
  attached.push(session.key);
  render(MemoryButton, { session });

  // The chat's saving root is asked for as it shows, and the switches follow it.
  if (folder !== GONE) {
    await waitFor(() => expect(memoryChat.rootOf(folder)).not.toBeNull());
  }

  return session;
}

/** A line of the popover, whole: a project's name is set in bold inside it. */
function line(panel: HTMLElement, text: string): HTMLElement {
  return within(panel).getByText((_, element) => element?.tagName === 'P' && element.textContent === text);
}

async function open(): Promise<HTMLElement> {
  await fireEvent.click(await screen.findByRole('button', { name: /^Memory/ }));

  return screen.findByRole('dialog', { name: 'Saving to memory' });
}

/** Saving is off everywhere, as it is for a user who turned nothing on. */
async function resetGuards(): Promise<void> {
  await memoryChat.loadGlobal();

  for (const kind of GUARD_KINDS) {
    await memoryChat.setGlobalPause(kind, false);
    await memoryChat.setEveryProject(kind, false);

    for (const repo of memoryChat.guards[kind].onRepos) {
      await memoryChat.setRepoOn(kind, repo, false);
    }
  }
}

/** Saving notes and scripts is turned on for the project. */
async function turnProjectOn(folder = ROOT): Promise<void> {
  for (const kind of GUARD_KINDS) {
    await memoryChat.setRepoOn(kind, folder, true);
  }
}

/** The folder's saving root is another one, as for a git worktree. */
function rootsTo(folder: string, root: string | null): void {
  const real = ipc.invoke.bind(ipc);

  vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: { folder?: string }) =>
    name === 'memory_saving_root' && args?.folder === folder ? Promise.resolve(root) : real(name as never, args as never)) as typeof ipc.invoke);
}

beforeEach(async () => {
  await startMockApp(PROJECT);
  harness.summaries = fixtures.harnesses;
  await memoryRemote.load();
  await resetGuards();
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

  it('is not shown when Harness Memory has save guards but cannot apply the opt-in ones', async () => {
    memoryRemote.value = { ...memoryRemote.value!, saveOptInSupported: false };

    await show();

    expect(screen.queryByRole('button', { name: /^Memory/ })).toBeNull();
  });

  it('is a red Memory disabled in a project saving was not turned on for, and a grey Memory once it is', async () => {
    await show();

    const disabled = await screen.findByRole('button', { name: /^Memory disabled/ });
    expect(disabled).toHaveClass('text-danger');
    expect(disabled).toHaveClass('bg-danger-wash');

    await turnProjectOn();

    const enabled = await screen.findByRole('button', { name: /^Memory$/ });
    expect(enabled).toHaveClass('text-fg');
    expect(enabled).toHaveClass('bg-surface-3');
    expect(enabled).not.toHaveClass('text-danger');
  });

  it('reads Memory once saving is turned on for the project, and keeps it in the list of projects', async () => {
    await show();
    const panel = await open();

    await fireEvent.click(within(panel).getByRole('switch', { name: 'Notes: This project' }));

    await waitFor(() => expect(screen.getByRole('button', { name: /^Memory$/ })).toBeInTheDocument());
    expect(memoryChat.guards.notes.onRepos).toEqual([ROOT]);
    expect(memoryChat.guards.scripts.onRepos).toEqual([]);
    expect(screen.getByRole('button', { name: /^Memory$/ })).not.toHaveClass('text-warning');
  });

  it('is on in a git worktree whose main repository is turned on', async () => {
    await turnProjectOn();
    rootsTo(WORKTREE, ROOT);

    await show(WORKTREE);

    expect(await screen.findByRole('button', { name: /^Memory$/ })).toBeInTheDocument();
    const panel = await open();
    expect(within(panel).getByRole('switch', { name: 'Notes: This project' })).toBeChecked();
    expect(within(panel).getAllByText((_, element) => element?.tagName === 'P' && element.textContent === 'Every chat in feature')).toHaveLength(2);
  });

  it('is on in every project once that is turned on', async () => {
    await memoryChat.setEveryProject('notes', true);

    await show('/home/demo/newly/opened');

    expect(await screen.findByRole('button', { name: /^Memory$/ })).toBeInTheDocument();
  });

  it('is Memory disabled when the user switched both kinds off in a project that has them on', async () => {
    await turnProjectOn();

    const session = await show();

    memoryChat.attach(session.key, { key: KEY, guards: { paused: false, notes: { on: false, ask: false }, scripts: { on: false, ask: false } } });

    expect(await screen.findByRole('button', { name: /^Memory disabled/ })).toHaveClass('text-danger');
  });

  it('is the amber Memory paused when saving is paused for every chat', async () => {
    await turnProjectOn();
    await memoryChat.setGlobalPause('notes', true);
    await memoryChat.setGlobalPause('scripts', true);

    await show();

    expect(await screen.findByRole('button', { name: /^Memory paused/ })).toHaveClass('text-warning');
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
  beforeEach(() => turnProjectOn());

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

  it('turns saving off for the project, for every chat in it, and says what it takes to turn it on', async () => {
    const session = await show();
    const panel = await open();

    await fireEvent.click(within(panel).getByRole('switch', { name: 'Scripts: This project' }));

    await waitFor(() => expect(memoryChat.guards.scripts.onRepos).toEqual([]));
    expect(memoryChat.guards.notes.onRepos).toEqual([ROOT]);
    expect(memoryChat.get(session.key)?.guards.scripts.on).toBe(true);
    expect(line(panel, 'Agents save no scripts in demo until you turn this on')).toBeInTheDocument();
    expect(within(panel).getByText('Off while demo is off')).toBeInTheDocument();
    expect(within(panel).getByRole('switch', { name: 'Scripts: This chat' })).toBeDisabled();

    await fireEvent.click(within(panel).getByRole('switch', { name: 'Scripts: This project' }));

    await waitFor(() => expect(memoryChat.guards.scripts.onRepos).toEqual([ROOT]));
  });

  it('shows the project switch on and fixed while saving is on for every project, and says where to change it', async () => {
    await memoryChat.setEveryProject('notes', true);
    await show();
    const panel = await open();

    const notes = within(panel).getByRole('switch', { name: 'Notes: This project' });
    expect(notes).toBeChecked();
    expect(notes).toBeDisabled();
    expect(within(panel).getByText('On for every project in Settings')).toBeInTheDocument();
    expect(within(panel).getByRole('switch', { name: 'Scripts: This project' })).toBeEnabled();
  });

  it('offers the project switch for a folder that is no git repository, which then stands for the folder itself', async () => {
    expect(fixtures.recentProjects[1].isGitRepo).toBe(false);
    await show(NOT_A_REPOSITORY);
    const panel = await open();

    const project = await within(panel).findByRole('switch', { name: 'Notes: This project' });
    expect(project).not.toBeChecked();

    await fireEvent.click(project);

    await waitFor(() => expect(memoryChat.guards.notes.onRepos).toContain(NOT_A_REPOSITORY));
    expect(within(panel).getByRole('switch', { name: 'Notes: This project' })).toBeChecked();
  });

  it('turns a worktree on by the folder the chat works in, whose key is the one its saves use', async () => {
    rootsTo(WORKTREE, ROOT);
    await memoryChat.setRepoOn('notes', ROOT, false);
    await show(WORKTREE);
    const panel = await open();

    await fireEvent.click(await within(panel).findByRole('switch', { name: 'Notes: This project' }));

    // The backend lists it under its saving root, and pins the key of the folder named here. The mock keeps what it was given.
    await waitFor(() => expect(memoryChat.guards.notes.onRepos).toEqual([WORKTREE]));
  });

  it('leaves out the project switches for a folder that is gone, and nothing is saved from its chat', async () => {
    rootsTo(GONE, null);
    await show(GONE);
    const panel = await open();

    expect(within(panel).queryByRole('switch', { name: 'Notes: This project' })).toBeNull();
    expect(within(panel).getByRole('switch', { name: 'Notes: This chat' })).toBeDisabled();
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
    expect(screen.getByRole('button', { name: /^Memory disabled/ })).toBeInTheDocument();
  });

  it('says why Check again and Pause are disabled when nothing can be saved in the project', async () => {
    await resetGuards();
    await show();
    const panel = await open();

    expect(within(panel).getByRole('button', { name: 'Check again' })).toBeDisabled();
    expect(within(panel).getByRole('button', { name: 'Pause' })).toBeDisabled();
    expect(within(panel).getByText('Nothing can be saved in this chat right now, so there is nothing to check or pause.')).toBeInTheDocument();
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
