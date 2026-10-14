import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi, type MockInstance } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
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
import { toasts } from '$lib/stores/toasts.svelte';
import { formatDateTime } from '$lib/utils/format';

import BranchesPanel from './BranchesPanel.svelte';
import { sessionTitle } from './session-format';
import { groupSessions } from './session-groups';
import SessionList from './SessionList.svelte';
import SessionsView from './SessionsView.svelte';
import Transcript from './Transcript.svelte';

const PROJECT = fixtures.recentProjects[0].path;
const SESSIONS = fixtures.sessions.list.omp;
const STATS = fixtures.sessions.messages.stats;
// The session a chat runs in the mock, and the empty copy a restart left of it.
const ROOT = SESSIONS[1];
const COPY = SESSIONS.find((summary) => summary.emptyCopy) as (typeof SESSIONS)[number];

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
  toasts.items = [];
  vi.restoreAllMocks();
  Reflect.deleteProperty(HTMLElement.prototype, 'clientWidth');
});

async function showSessions(): Promise<void> {
  nav.go('sessions');
  render(SessionsView);
  await waitFor(() => expect(screen.getByRole('list', { name: 'Session transcript' })).toBeInTheDocument());
}

const sessionButtons = () => [...document.querySelectorAll<HTMLElement>('[data-session]')];
const sessionButton = (id: string) => document.querySelector<HTMLElement>(`[data-session="${id}"]`) as HTMLElement;
const rowOf = (id: string) => sessionButton(id).closest('[role="listitem"]') as HTMLElement;
const deleteCalls = (invoke: MockInstance<typeof ipc.invoke>) => invoke.mock.calls.filter(([name]) => name === 'session_delete').map(([, args]) => args);

describe('session list', () => {
  it('lists the saved sessions newest first, empty copies folded away, titled by name or first prompt', async () => {
    await showSessions();

    const shown = SESSIONS.filter((summary) => !summary.emptyCopy);
    const buttons = sessionButtons();
    expect(buttons.map((button) => button.dataset.session)).toEqual(shown.map((summary) => summary.ref.id));

    for (const [index, summary] of shown.entries()) {
      expect(buttons[index]).toHaveAccessibleName(sessionTitle(summary));
      expect(rowOf(summary.ref.id)).toHaveTextContent(summary.model ?? '');
    }

    const untitled = SESSIONS.find((summary) => summary.title === null);
    expect(untitled).toBeDefined();
    expect(sessionTitle(untitled as (typeof SESSIONS)[number])).not.toBe('Untitled');
  });

  it('searches titles, first prompts and models', async () => {
    await showSessions();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search sessions' }), { target: { value: 'release' } });

    expect(sessionButtons()).toHaveLength(1);
    expect(sessionButtons()[0]).toHaveAccessibleName('Release checklist');

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

  it('folds the empty copies of a session under it, and lists them indented when asked', async () => {
    await showSessions();
    expect(sessionButton(COPY.ref.id)).toBeNull();

    const chip = within(rowOf(ROOT.ref.id)).getByRole('button', { name: '1 copy' });
    expect(chip).toHaveAttribute('aria-expanded', 'false');

    await fireEvent.click(chip);

    expect(chip).toHaveAttribute('aria-expanded', 'true');
    expect(sessionButton(COPY.ref.id)).toBeTruthy();
    // The copy is a row like any other: its own button and its own menu.
    expect(within(rowOf(COPY.ref.id)).getByRole('button', { name: 'Session actions' })).toBeInTheDocument();
    expect(sessionButtons().findIndex((button) => button.dataset.session === COPY.ref.id)).toBe(sessionButtons().findIndex((button) => button.dataset.session === ROOT.ref.id) + 1);

    await fireEvent.click(chip);

    expect(sessionButton(COPY.ref.id)).toBeNull();
  });
});

describe('session list keyboard', () => {
  it('moves the selection with the arrow keys from a row, but not from inside its open menu', async () => {
    await showSessions();
    const shown = SESSIONS.filter((summary) => !summary.emptyCopy);
    await fireEvent.click(sessionButton(shown[0].ref.id));
    expect(sessionButton(shown[0].ref.id)).toHaveAttribute('aria-current', 'true');

    await fireEvent.click(within(rowOf(shown[0].ref.id)).getByRole('button', { name: 'Session actions' }));
    await fireEvent.keyDown(await screen.findByRole('menuitem', { name: 'Delete' }), { key: 'ArrowDown' });

    expect(sessionButton(shown[0].ref.id)).toHaveAttribute('aria-current', 'true');
    expect(sessionButton(shown[1].ref.id)).not.toHaveAttribute('aria-current');

    await fireEvent.keyDown(sessionButton(shown[0].ref.id), { key: 'ArrowDown' });

    expect(sessionButton(shown[1].ref.id)).toHaveAttribute('aria-current', 'true');
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

  it('forks from a user message with the button beside it', async () => {
    await showSessions();
    const messages = await screen.findAllByLabelText('Your message');
    const forks = screen.getAllByRole('button', { name: 'Fork from here' });
    expect(forks).toHaveLength(messages.length);

    await fireEvent.click(forks[0]);

    await waitFor(() => expect(nav.current).toBe('chat'));
    expect(agentSessions.sessions).toHaveLength(1);
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

describe('session details', () => {
  it('shows the totals, the models and the facts of the session in four cards', async () => {
    await showSessions();
    await fireEvent.click(sessionButton(ROOT.ref.id));

    const details = await screen.findByRole('complementary', { name: 'Session details' });
    await waitFor(() => expect(within(details).getByText(String(STATS?.messages))).toBeInTheDocument());
    await waitFor(() => expect(within(details).getByRole('button', { name: fixtures.sessions.filesTouched[0] })).toBeInTheDocument());

    expect([...details.querySelectorAll('h3')].map((heading) => heading.textContent?.replace(/\s+/g, ' ').trim())).toEqual(['Overview', 'Models', 'Session', `Files touched · ${fixtures.sessions.filesTouched.length}`]);
    expect(within(details).getByText(String(STATS?.toolCalls))).toBeInTheDocument();
    expect(within(details).getByText('12.8K')).toBeInTheDocument();
    expect(within(details).getByText('$0.0750')).toBeInTheDocument();

    for (const model of STATS?.models ?? []) {
      expect(within(details).getByText(model.model)).toBeInTheDocument();
      expect(within(details).getByText(`${model.turns} turns`)).toBeInTheDocument();
    }

    expect(within(details).getByText(formatDateTime(STATS?.startedAt ?? 0))).toBeInTheDocument();
    expect(within(details).getByText('19 min')).toBeInTheDocument();
    expect(within(details).getByText('Forks').nextElementSibling).toHaveTextContent('1');
  });

  it('moves into a sheet behind a Details button when the pane is narrow', async () => {
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 820 });
    await showSessions();

    expect(screen.queryByRole('complementary', { name: 'Session details' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Details' }));

    const sheet = await screen.findByRole('dialog', { name: 'Session details' });
    expect(within(sheet).getByText('Overview')).toBeInTheDocument();

    await fireEvent.keyDown(sheet, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Session details' })).toBeNull());
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
    expect(screen.queryByRole('button', { name: 'Fork from here' })).toBeNull();
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

describe('deleting sessions', () => {
  async function confirmDelete(label: string): Promise<void> {
    await fireEvent.click(await screen.findByRole('button', { name: label }));
  }

  it('refuses to delete a session an open chat runs', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const chat = await agentSessions.create('omp', PROJECT);
    await waitFor(() => expect(chat?.chat.session?.sessionId).toBe(ROOT.ref.id));
    await showSessions();

    await fireEvent.click(within(rowOf(ROOT.ref.id)).getByRole('button', { name: 'Session actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete' }));

    expect(toasts.items.at(-1)?.title).toBe('This session is open in a chat');
    expect(screen.queryByRole('button', { name: 'Delete session' })).toBeNull();
    expect(deleteCalls(invoke)).toHaveLength(0);
  });

  it('deletes a session from its row menu once confirmed, and the row disappears', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    const target = SESSIONS[2];
    await showSessions();

    await fireEvent.click(within(rowOf(target.ref.id)).getByRole('button', { name: 'Session actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete' }));
    expect(await screen.findByText('Delete this session?')).toBeInTheDocument();
    expect(deleteCalls(invoke)).toHaveLength(0);

    await confirmDelete('Delete session');

    await waitFor(() => expect(sessionButton(target.ref.id)).toBeNull());
    expect(deleteCalls(invoke)).toEqual([{ session: target.ref, project: PROJECT, emptyCopyOnly: false }]);
  });

  it('keeps the session when the confirmation is cancelled', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    await showSessions();

    await fireEvent.click(within(rowOf(ROOT.ref.id)).getByRole('button', { name: 'Session actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete' }));
    await confirmDelete('Keep it');

    expect(deleteCalls(invoke)).toHaveLength(0);
    expect(sessionButton(ROOT.ref.id)).toBeTruthy();
  });

  it('deletes the shown session from the header menu and moves on to the next one', async () => {
    const target = SESSIONS[0];
    await showSessions();
    await fireEvent.click(sessionButton(target.ref.id));
    expect(await screen.findByRole('heading', { level: 2, name: sessionTitle(target) })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete' }));
    await confirmDelete('Delete session');

    await waitFor(() => expect(sessionButton(target.ref.id)).toBeNull());
    expect(screen.queryByRole('heading', { level: 2, name: sessionTitle(target) })).toBeNull();
    expect(await screen.findByRole('heading', { level: 2, name: sessionTitle(ROOT) })).toBeInTheDocument();
  });

  it('cleans up the empty copies after one confirmation, naming only copies', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    await showSessions();
    expect(screen.getByText('1 empty copy')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Clean up' }));
    expect(await screen.findByText('Delete 1 empty copy?')).toBeInTheDocument();
    await confirmDelete('Delete copies');

    await waitFor(() => expect(screen.queryByRole('button', { name: 'Clean up' })).toBeNull());
    expect(deleteCalls(invoke)).toEqual([{ session: COPY.ref, project: PROJECT, emptyCopyOnly: true }]);
    expect(toasts.items.at(-1)?.title).toBe('Deleted 1 empty copy');
    expect(within(rowOf(ROOT.ref.id)).queryByRole('button', { name: /cop/ })).toBeNull();
  });
});

describe('in Polish', () => {
  function copiesOf(count: number) {
    const root = { ...ROOT, ref: { harness: 'omp', id: 'root' }, emptyCopy: false } as (typeof SESSIONS)[number];
    const copies = Array.from({ length: count }, (_, index) => ({ ...COPY, ref: { harness: 'omp', id: `copy-${index}` }, parentSession: 'root' }) as (typeof SESSIONS)[number]);

    return groupSessions([root, ...copies]);
  }

  it('translates the session view', async () => {
    await i18n.load('pl');
    nav.go('sessions');
    render(SessionsView);

    expect(await screen.findByRole('searchbox', { name: 'Szukaj sesji' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('list', { name: 'Transkrypcja sesji' })).toBeInTheDocument());

    expect((await screen.findAllByRole('button', { name: 'Rozgałęź stąd' })).length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: 'Wznów w czacie' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Więcej działań' })).toBeInTheDocument();
    expect(await screen.findByRole('complementary', { name: 'Szczegóły sesji' })).toBeInTheDocument();
  });

  it('counts copies with the Polish plural forms', async () => {
    await i18n.load('pl');
    const props = { selectedId: null, onselect: () => {}, ondelete: () => {}, oncleanup: () => {} };

    for (const [count, chip, banner] of [
      [1, '1 kopia', '1 pusta kopia'],
      [3, '3 kopie', '3 puste kopie'],
      [5, '5 kopii', '5 pustych kopii'],
    ] as const) {
      const { unmount } = render(SessionList, { groups: copiesOf(count), ...props });

      expect(screen.getByRole('button', { name: chip })).toBeInTheDocument();
      expect(screen.getByText(banner)).toBeInTheDocument();
      unmount();
    }
  });
});
