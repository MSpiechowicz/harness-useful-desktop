import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';

import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import { ipc } from '$lib/ipc/client';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { ROUTINES_WORKSPACE } from '$lib/ipc/mock/routine-fixtures';

import { PROJECT } from '../../../tests/chat-helpers';
import ChatPane from '../chat/ChatPane.svelte';
import { startMockApp } from '../../../tests/mock-app';
import { RunNotices } from './routine-notices';
import { openRunChat, openRunSession, showRoutine } from './routine-open';
import { openLiveRoutineSession, openRequestedRun } from './routine-request';
import { routines } from './routines-data.svelte';

beforeAll(() => startMockApp(PROJECT));

beforeEach(async () => {
  routines.reset();
  routines.follow(PROJECT);
  await routines.refresh();
});

afterEach(async () => {
  for (const session of [...agentSessions.sessions]) {
    await agentSessions.stop(session.key);
  }

  nav.go('chat');
  viewState.reset('routines');
  toasts.items = [];
});

/** The run that goes on and asks the user something. */
function asking(): RoutineRun {
  return [...routines.running.values()].find((run) => run.needsYou) as RoutineRun;
}

describe('opening the chat of a run', () => {
  it('opens the run as a tab, shows it in Chat and focuses it', async () => {
    const run = asking();
    nav.go('routines');

    await openRunChat(run);

    expect(nav.current).toBe('chat');
    expect(agentSessions.active?.routine).toEqual({ runId: run.id, routineId: run.routineId, name: run.routineName });
  });

  it('shows the saved session in Sessions when the run ended before the chat could be opened', async () => {
    const run = { ...asking(), id: 'f'.repeat(32) };

    await openRunChat(run);

    expect(agentSessions.sessions).toHaveLength(0);
    expect(nav.current).toBe('sessions');
    expect(viewState.sessionId).toBe(run.session?.id);
  });

  it('keeps the folder of a global routine in the routines folder entry, and of a project routine in the project', () => {
    const session = { harness: 'omp', id: 'e'.repeat(32) } as const;

    openRunSession({ session, project: null });

    expect(nav.current).toBe('sessions');
    expect(viewState.sessionsFolder).toBe('routines');
    expect(viewState.sessionId).toBe(session.id);
    expect(viewState.sessionAsked).toBe(true);

    openRunSession({ session, project: PROJECT });

    expect(viewState.sessionsFolder).toBe('project');
  });

  it('opens Sessions as it is when the run left no session', () => {
    viewState.sessionsFolder = 'project';

    openRunSession({ session: null, project: null });

    expect(nav.current).toBe('sessions');
    expect(viewState.sessionsFolder).toBe('project');
    expect(viewState.sessionId).toBeNull();
  });

  it('opens the chat the backend asked for, after a click on a notification', async () => {
    const run = asking();

    await openRequestedRun(run.id);

    expect(nav.current).toBe('chat');
    expect(agentSessions.active?.routine?.runId).toBe(run.id);
  });

  it('reads the runs again when the request is faster than the list', async () => {
    const run = asking();
    routines.running = new Map();

    await openRequestedRun(run.id);

    expect(agentSessions.active?.routine?.runId).toBe(run.id);
  });

  it('opens the Routines view when the requested run is already over', async () => {
    await openRequestedRun('f'.repeat(32));

    expect(nav.current).toBe('routines');
    expect(agentSessions.sessions).toHaveLength(0);
  });

  it('opens the chat from the toast that says a run needs the user', async () => {
    const run = asking();
    const toast = new RunNotices(() => true).toastFor({ kind: 'needsYou', runId: run.id, routineId: run.routineId, name: run.routineName });

    expect(toast?.action?.label).toBe('Open chat');
    toast?.action?.run();
    await expect.poll(() => agentSessions.active?.routine?.runId).toBe(run.id);

    expect(nav.current).toBe('chat');
  });
});

describe('opening the session of a routine from Sessions', () => {
  it('attaches to a run that goes on instead of resuming its session, which the run holds', async () => {
    const run = asking();
    const calls: string[] = [];
    const real = ipc.invoke.bind(ipc);
    ipc.invoke = ((name: string, args: unknown) => (calls.push(name), real(name as never, args as never))) as typeof ipc.invoke;

    try {
      const handled = await openLiveRoutineSession({ routine: { routineId: run.routineId, runId: run.id, name: run.routineName } });

      expect(handled).toBe(true);
      expect(calls).toContain('routine_run_attach');
      expect(calls).not.toContain('agent_start');
      expect(nav.current).toBe('chat');
    } finally {
      ipc.invoke = real;
    }
  });

  it('leaves a session of a finished run, and any other, to be resumed', async () => {
    expect(await openLiveRoutineSession({ routine: { routineId: 'c'.repeat(32), runId: 'd'.repeat(32), name: 'Done' } })).toBe(false);
    expect(await openLiveRoutineSession({})).toBe(false);
    expect(agentSessions.sessions).toHaveLength(0);
  });
});

describe('showing a routine', () => {
  it('opens it in the Routines view', () => {
    showRoutine('c'.repeat(32));

    expect(nav.current).toBe('routines');
    expect(viewState.routineId).toBe('c'.repeat(32));
  });
});

describe('the question of a run in its chat tab', () => {
  it('shows as the same dialog as in any chat, and the answer goes to the run', async () => {
    const run = asking();
    const calls: unknown[] = [];
    const real = ipc.invoke.bind(ipc);
    ipc.invoke = ((name: string, args: unknown) => (name === 'agent_ui_respond' && calls.push(args), real(name as never, args as never))) as typeof ipc.invoke;

    try {
      await openRunChat(run);
      const session = agentSessions.active!;
      render(ChatPane, { session });

      const dialog = await screen.findByRole('dialog', undefined, { timeout: 5000 });
      await fireEvent.click(within(dialog).getAllByRole('button')[0]);

      await waitFor(() => expect(calls).toHaveLength(1));
      expect(calls[0]).toMatchObject({ key: session.key });
    } finally {
      ipc.invoke = real;
    }
  });
});

describe('the folders of global routines', () => {
  it('counts the routines folder and each folder below it, and nothing else', () => {
    routines.workspace = ROUTINES_WORKSPACE;

    expect(routines.inWorkspace(ROUTINES_WORKSPACE)).toBe(true);
    expect(routines.inWorkspace(`${ROUTINES_WORKSPACE}/${'c'.repeat(32)}`)).toBe(true);
    expect(routines.inWorkspace(`${ROUTINES_WORKSPACE}-other`)).toBe(false);
    expect(routines.inWorkspace(PROJECT)).toBe(false);

    routines.workspace = null;
    expect(routines.inWorkspace(ROUTINES_WORKSPACE)).toBe(false);
  });
});
