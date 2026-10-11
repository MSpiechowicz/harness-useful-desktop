import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { createMockTransport } from '$lib/ipc/mock/transport';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { SessionDetail } from '$lib/stores/sessions.svelte';

import BranchesPanel from './BranchesPanel.svelte';
import { sessionTitle } from './session-format';
import SessionsView from './SessionsView.svelte';
import Transcript from './Transcript.svelte';

const PROJECT = fixtures.recentProjects[0].path;
const SESSIONS = fixtures.sessions.list.omp;
const TREE = fixtures.sessions.tree;

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  // happy-dom lays nothing out: give every element a 640px viewport so virtual lists render a window of rows.
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 640 });

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  await project.open(PROJECT);
});

afterEach(async () => {
  for (const session of [...agentSessions.sessions]) {
    await agentSessions.stop(session.key);
  }

  git.activate(null);
  nav.go('chat');
});

async function showSessions(): Promise<void> {
  nav.go('sessions');
  render(SessionsView);
  await waitFor(() => expect(screen.getByRole('list', { name: 'Session branches' })).toBeInTheDocument());
}

const sessionButtons = () => [...document.querySelectorAll<HTMLElement>('[data-session]')];
const treeRow = (id: string) => document.querySelector<HTMLElement>(`[data-row-key="${id}"]`) as HTMLElement;

describe('session list', () => {
  it('lists the saved sessions newest first, titled by name or first prompt', async () => {
    await showSessions();

    const buttons = sessionButtons();
    expect(buttons.map((button) => button.dataset.session)).toEqual(SESSIONS.map((summary) => summary.ref.id));

    for (const [index, summary] of SESSIONS.entries()) {
      expect(buttons[index]).toHaveTextContent(sessionTitle(summary));
      expect(buttons[index]).toHaveTextContent(summary.model ?? '');
    }

    const untitled = SESSIONS.find((summary) => summary.title === null);
    expect(untitled).toBeDefined();
    expect(sessionTitle(untitled as (typeof SESSIONS)[number])).not.toBe('Untitled');
  });

  it('searches titles, first prompts and models', async () => {
    await showSessions();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search sessions' }), { target: { value: 'release' } });

    expect(sessionButtons()).toHaveLength(1);
    expect(sessionButtons()[0]).toHaveTextContent('Release checklist');

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search sessions' }), { target: { value: 'no such session anywhere' } });

    expect(sessionButtons()).toHaveLength(0);
    expect(screen.getByText('No session matches that search.')).toBeInTheDocument();
  });

  it('asks for a project first', async () => {
    await project.close();
    render(SessionsView);

    expect(await screen.findByText('Open a project to see its sessions')).toBeInTheDocument();

    await project.open(PROJECT);
  });
});

describe('branch tree', () => {
  it('draws every entry with the active path marked and branch points counted', async () => {
    await showSessions();

    for (const node of TREE.nodes) {
      expect(treeRow(node.id)).toBeTruthy();
      expect(treeRow(node.id).dataset.activePath).toBe(String(node.onActivePath));
    }

    expect(screen.getByText(`${TREE.branchCount} branch points`, { exact: false })).toBeInTheDocument();
    expect(treeRow(TREE.leafId ?? '')).toHaveAttribute('aria-current', 'true');
    expect(treeRow(TREE.nodes.find((node) => node.childCount > 1)?.id ?? '')).toHaveTextContent(/\d branches?/);
  });

  it('moves the selection with the arrow keys', async () => {
    await showSessions();
    const leaf = treeRow(TREE.leafId ?? '');
    leaf.focus();

    await fireEvent.keyDown(leaf, { key: 'ArrowUp' });

    const parent = TREE.nodes[TREE.nodes.length - 2];
    await waitFor(() => expect(treeRow(parent.id)).toHaveAttribute('aria-current', 'true'));
    expect(leaf).not.toHaveAttribute('aria-current');

    await fireEvent.keyDown(treeRow(parent.id), { key: 'Home' });

    await waitFor(() => expect(treeRow(TREE.nodes[0].id)).toHaveAttribute('aria-current', 'true'));
  });

  it('forks only from one of the user\'s own messages', async () => {
    await showSessions();
    const fork = screen.getByRole('button', { name: 'Fork from here' });
    const assistant = TREE.nodes.find((node) => node.kind === 'assistant') as (typeof TREE.nodes)[number];
    const user = TREE.nodes.find((node) => node.kind === 'user') as (typeof TREE.nodes)[number];

    await fireEvent.click(treeRow(assistant.id));
    expect(fork).toBeDisabled();

    await fireEvent.click(treeRow(user.id));
    expect(fork).toBeEnabled();

    await fireEvent.click(fork);

    await waitFor(() => expect(nav.current).toBe('chat'));
    expect(agentSessions.sessions).toHaveLength(1);
  });
});

describe('session actions', () => {
  it('resumes the selected session in a new chat', async () => {
    await showSessions();

    await fireEvent.click(screen.getByRole('button', { name: 'Resume in chat' }));

    await waitFor(() => expect(nav.current).toBe('chat'));
    expect(agentSessions.sessions).toHaveLength(1);
    expect(agentSessions.active?.info.project).toBe(PROJECT);
  });

  it('opens Changes limited to a touched file, or to all of them', async () => {
    await showSessions();
    const touched = fixtures.sessions.filesTouched;
    const group = await screen.findByRole('group', { name: 'Files touched' });

    await fireEvent.click(within(group).getByRole('button', { name: touched[0] }));

    expect(nav.current).toBe('changes');
    expect(git.filter).toEqual({ label: 'this session', paths: [touched[0]] });

    await fireEvent.click(within(group).getByRole('button', { name: 'View changes' }));

    expect(git.filter?.paths).toEqual(touched);
  });
});

describe('transcript', () => {
  it('reads the active path and loads further pages on request', async () => {
    const detail = new SessionDetail(createMockTransport(defaultScenario(), 0), 5);
    const total = fixtures.sessions.messages.messages.length;
    detail.open(SESSIONS[1].ref, PROJECT);

    render(Transcript, { detail, project: PROJECT });
    await waitFor(() => expect(screen.getByRole('list', { name: 'Session transcript' })).toBeInTheDocument());

    expect(await screen.findByText(`5 of ${total} messages`)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Load more messages' }));

    await waitFor(() => expect(detail.messages.length).toBeGreaterThan(5));
    expect(screen.getByRole('list', { name: 'Session transcript' }).querySelectorAll('[role="listitem"]').length).toBeGreaterThan(0);
  });

  it('shows the first user message as read-only chat content', async () => {
    const detail = new SessionDetail(createMockTransport(defaultScenario(), 0));
    detail.open(SESSIONS[1].ref, PROJECT);

    render(Transcript, { detail, project: PROJECT });

    const firstPrompt = fixtures.sessions.messages.messages.find((message) => message.role === 'user');
    const text = firstPrompt?.blocks.find((block) => block.type === 'text');
    expect((await screen.findAllByLabelText('Your message'))[0]).toHaveTextContent(text?.type === 'text' ? text.text : '');
    expect(screen.queryByRole('textbox')).toBeNull();
  });
});

describe('git branches', () => {
  it('shows the current branch with its distance from upstream, then the others with their last commit', async () => {
    git.activate(PROJECT);
    render(BranchesPanel);

    const current = fixtures.git.branches.find((branch) => branch.isCurrent);
    const localList = await waitFor(() => screen.getByRole('list', { name: 'Local branches' }));

    const row = within(localList).getByText(current?.name ?? '').closest('li') as HTMLElement;
    expect(row).toHaveAttribute('aria-current', 'true');
    expect(row).toHaveTextContent('Current');
    expect(row).toHaveTextContent(`↑${current?.ahead}`);
    expect(row).toHaveTextContent(`↓${current?.behind}`);
    expect(row).toHaveTextContent(current?.lastCommit?.summary ?? '');

    expect(screen.getByRole('list', { name: 'Remote branches' }).querySelectorAll('li')).toHaveLength(fixtures.git.branches.filter((branch) => branch.isRemote).length);
  });

  it('collapses', async () => {
    git.activate(PROJECT);
    render(BranchesPanel);
    await waitFor(() => expect(screen.getByRole('list', { name: 'Local branches' })).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: /Git branches/ }));

    expect(screen.queryByRole('list', { name: 'Local branches' })).toBeNull();
  });

  it('says so when the project is not a repository', async () => {
    const plain = fixtures.recentProjects.find((candidate) => !candidate.isGitRepo && candidate.exists);
    await project.open(plain?.path ?? '');
    git.activate(plain?.path ?? null);
    render(BranchesPanel);

    expect(await screen.findByText('This project is not a git repository.')).toBeInTheDocument();

    await project.open(PROJECT);
  });
});
