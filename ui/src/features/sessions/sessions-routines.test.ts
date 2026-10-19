import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { ROUTINES_WORKSPACE } from '$lib/ipc/mock/routine-fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { routineSessionList, sessionList } from '$lib/stores/sessions.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { openRunSession } from '../routines/routine-open';
import { routines } from '../routines/routines-data.svelte';
import SessionsView from './SessionsView.svelte';

const PROJECT = fixtures.recentProjects[0].path;
const SESSIONS = fixtures.sessions.list.omp;
// The mock lists the second session as the one the asking routine run left, and the fourth as one a finished run left.
const LIVE = SESSIONS[1];

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  // happy-dom lays nothing out: give every element a 640px viewport so virtual lists render a window of rows.
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 640 });

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  await project.open(PROJECT);
});

beforeEach(async () => {
  routines.reset();
  viewState.reset('sessions');
  sessionList.sync(null, null);
  routineSessionList.sync(null, null);
  routines.follow(PROJECT);
  routines.start();
  await waitFor(() => expect(routines.workspace).toBe(ROUTINES_WORKSPACE));
  await waitFor(() => expect(routines.running.size).toBeGreaterThan(0));
});

afterEach(async () => {
  for (const session of [...agentSessions.sessions]) {
    await agentSessions.stop(session.key);
  }

  git.activate(null);
  nav.go('chat');
  viewState.reset('routines');
  toasts.items = [];
  vi.restoreAllMocks();
});

async function showSessions(): Promise<void> {
  nav.go('sessions');
  render(SessionsView);
  await waitFor(() => expect(screen.getByRole('list', { name: 'Session transcript' })).toBeInTheDocument());
}

const rowOf = (id: string) => document.querySelector<HTMLElement>(`[data-session="${id}"]`)?.closest('[role="listitem"]') as HTMLElement;

describe('the Routine chip of a session', () => {
  it('names the routine on the row of a session a run started, and on no other', async () => {
    await showSessions();

    expect(within(rowOf(LIVE.ref.id)).getByRole('button', { name: 'Routine · Triage new issues' })).toBeInTheDocument();
    expect(within(rowOf(SESSIONS[0].ref.id)).queryByText(/^Routine ·/)).not.toBeInTheDocument();
  });

  it('shows the routine in the Routines view when clicked, without opening the session', async () => {
    await showSessions();
    const chip = within(rowOf(LIVE.ref.id)).getByRole('button', { name: 'Routine · Triage new issues' });
    const run = [...routines.running.values()].find((candidate) => candidate.needsYou);

    await fireEvent.click(chip);

    expect(nav.current).toBe('routines');
    expect(viewState.routineId).toBe(run?.routineId);
  });
});

describe('the folder of the global routines', () => {
  async function chooseRoutinesFolder(): Promise<void> {
    await fireEvent.click(await screen.findByRole('button', { name: /^Folder: / }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Global routines' }));
  }

  it('is offered while it holds sessions, next to the project, and lists them with their routines', async () => {
    await showSessions();

    const picker = await screen.findByRole('button', { name: /^Folder: / });
    expect(picker).toHaveAccessibleName(`Folder: ${project.active?.name}`);

    await chooseRoutinesFolder();

    await waitFor(() => expect(screen.getByRole('button', { name: /^Folder: Global routines$/ })).toBeInTheDocument());
    expect((await screen.findAllByRole('button', { name: 'Routine · Morning review' })).length).toBeGreaterThan(0);
    expect(screen.queryByRole('button', { name: 'Routine · Triage new issues' })).not.toBeInTheDocument();
  });

  it('reads the sessions of the routines folder, and goes back to the project’s', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    await showSessions();
    await chooseRoutinesFolder();

    await waitFor(() => expect(invoke.mock.calls.some(([name, args]) => name === 'sessions_list' && (args as { project: string }).project === ROUTINES_WORKSPACE)).toBe(true));

    await fireEvent.click(screen.getByRole('button', { name: /^Folder: / }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: project.active?.name }));

    await waitFor(() => expect(screen.getByRole('button', { name: `Folder: ${project.active?.name}` })).toBeInTheDocument());
    expect(viewState.sessionsFolder).toBe('project');
  });

  it('is not offered when it holds no sessions', async () => {
    const real = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) =>
      name === 'sessions_list' && (args as { project: string }).project === ROUTINES_WORKSPACE ? Promise.resolve([]) : real(name as never, args as never)) as typeof ipc.invoke);

    await showSessions();
    await waitFor(() => expect(routineSessionList.status).toBe('ready'));

    expect(screen.queryByRole('button', { name: /^Folder: / })).not.toBeInTheDocument();
  });

  it('is not offered before any global routine ran', async () => {
    routines.workspace = null;
    const real = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => (name === 'routines_workspace' ? Promise.resolve(null) : real(name as never, args as never))) as typeof ipc.invoke);

    await showSessions();

    expect(screen.queryByRole('button', { name: /^Folder: / })).not.toBeInTheDocument();
  });
});

describe('opening a routine session that is still running', () => {
  it('attaches to its run in Chat instead of resuming the session', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    await showSessions();
    await fireEvent.click(document.querySelector<HTMLElement>(`[data-session="${LIVE.ref.id}"]`) as HTMLElement);

    await fireEvent.click(await screen.findByRole('button', { name: 'Resume in chat' }));

    await waitFor(() => expect(nav.current).toBe('chat'));
    expect(agentSessions.active?.routine?.name).toBe('Triage new issues');
    expect(invoke.mock.calls.some(([name]) => name === 'routine_run_attach')).toBe(true);
    expect(invoke.mock.calls.some(([name]) => name === 'agent_start')).toBe(false);
  });

  it('resumes the session of a finished run as usual', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    await showSessions();
    await fireEvent.click(document.querySelector<HTMLElement>(`[data-session="${SESSIONS[0].ref.id}"]`) as HTMLElement);

    await fireEvent.click(await screen.findByRole('button', { name: 'Resume in chat' }));

    await waitFor(() => expect(invoke.mock.calls.some(([name]) => name === 'agent_start')).toBe(true));
    expect(invoke.mock.calls.some(([name]) => name === 'routine_run_attach')).toBe(false);
  });
});

describe('opening the session of a run from the Routines view', () => {
  const rows = () => [...document.querySelectorAll<HTMLElement>('[data-session]')];
  const current = () => document.querySelector<HTMLElement>('[data-session][aria-current="true"]')?.dataset.session;

  async function routineSessions() {
    const listed = await ipc.invoke('sessions_list', { harness: 'omp', project: ROUTINES_WORKSPACE });

    expect(listed.length).toBeGreaterThan(1);

    return listed;
  }

  it('shows the session of a global run in the routines folder, though that folder was not known yet', async () => {
    const target = (await routineSessions()).at(-1)!;
    routines.workspace = null;

    openRunSession({ session: { harness: 'omp', id: target.ref.id }, project: null });
    await showSessions();

    await waitFor(() => expect(screen.getByRole('button', { name: /^Folder: Global routines$/ })).toBeInTheDocument());
    await waitFor(() => expect(current()).toBe(target.ref.id));
    expect(viewState.sessionAsked).toBe(false);
  });

  it('shows the session of a project run in the project, not in the routines folder', async () => {
    viewState.sessionsFolder = 'routines';

    openRunSession({ session: LIVE.ref, project: PROJECT });
    await showSessions();

    await waitFor(() => expect(current()).toBe(LIVE.ref.id));
    expect(screen.getByRole('button', { name: `Folder: ${project.active?.name}` })).toBeInTheDocument();
  });

  it('says the session is missing, instead of showing another one, when it is not in the folder', async () => {
    openRunSession({ session: { harness: 'omp', id: 'gone-session' }, project: PROJECT });
    nav.go('sessions');
    render(SessionsView);

    expect(await screen.findByText('This session was not found')).toBeInTheDocument();
    expect(current()).toBeUndefined();
    expect(rows().length).toBeGreaterThan(0);
    expect(screen.queryByRole('list', { name: 'Session transcript' })).not.toBeInTheDocument();
  });

  it('goes back to the project when no global routine ever ran and the routines folder was asked for', async () => {
    routines.workspace = null;
    const real = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => (name === 'routines_workspace' ? Promise.resolve(null) : real(name as never, args as never))) as typeof ipc.invoke);

    openRunSession({ session: { harness: 'omp', id: SESSIONS[0].ref.id }, project: null });
    await showSessions();

    expect(viewState.sessionsFolder).toBe('project');
  });
});
