import { afterEach, describe, expect, it, vi } from 'vitest';

import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';

import { PROJECT, storeWithMock } from '../../../tests/chat-helpers';
import { AgentSessionsStore } from './agentSession.svelte';
import { ToastsStore } from './toasts.svelte';
import { saveWorkspace } from './workspace.svelte';

afterEach(() => {
  localStorage.clear();
});

/** What the next launch does: a fresh copy of the module restores once. */
async function relaunch() {
  vi.resetModules();

  return import('./workspace.svelte');
}

async function liveRun(backend: ReturnType<typeof storeWithMock>): Promise<RoutineRun> {
  const page = await backend.transport.invoke('routine_runs', { query: { routineId: null, status: 'running', offset: 0, limit: 20 } });

  return page.runs.find((candidate) => candidate.needsYou) as RoutineRun;
}

describe('a routine tab after the page was reloaded', () => {
  it('attaches to its run again while the run goes on, without resuming the session', async () => {
    const before = storeWithMock();
    const run = await liveRun(before);
    const tab = (await before.store.attachRoutine({ runId: run.id, routineId: run.routineId, name: run.routineName }))!;
    saveWorkspace(before.store);

    const { restoreWorkspace } = await relaunch();
    // The backend outlives the page: the same mock serves the reloaded one, which has no tabs yet.
    const after = { store: new AgentSessionsStore(before.transport, new ToastsStore()), of: before.of };

    expect(await restoreWorkspace(after.store)).toBe(true);

    // The first attach was the tab before the reload, the second is the one the reload made.
    expect(after.of('routine_run_attach').map((call) => call.runId)).toEqual([run.id, run.id]);
    expect(after.of('agent_start')).toHaveLength(0);
    expect(after.store.sessions).toHaveLength(1);
    expect(after.store.sessions[0].routine).toEqual({ runId: run.id, routineId: run.routineId, name: run.routineName });
    expect(after.store.activeKey).toBe(after.store.sessions[0].key);
    expect(tab.routine?.runId).toBe(run.id);
  });

  it('falls back to the transcript when the run is over: the session is resumed like any chat', async () => {
    const run = { runId: 'f'.repeat(32), routineId: 'c'.repeat(32), name: 'Gone' };
    localStorage.setItem('harness:workspace', JSON.stringify({ view: 'chat', chats: [{ harness: 'omp', sessionId: 'saved-1', project: PROJECT, routine: run }], active: 0 }));
    const { restoreWorkspace } = await relaunch();
    const after = storeWithMock();

    expect(await restoreWorkspace(after.store)).toBe(true);

    expect(after.of('routine_run_attach')).toHaveLength(1);
    expect(after.of('agent_start')[0]).toMatchObject({ request: { harness: 'omp', project: PROJECT, resume: { harness: 'omp', id: 'saved-1' } } });
    expect(after.store.sessions).toHaveLength(1);
    expect(after.store.sessions[0].routine).toBeNull();
    expect(after.notifications.items).toHaveLength(0);
  });

  it('leaves a tab with neither a live run nor a session out', async () => {
    const run = { runId: 'f'.repeat(32), routineId: 'c'.repeat(32), name: 'Gone' };
    localStorage.setItem('harness:workspace', JSON.stringify({ view: 'chat', chats: [{ harness: 'omp', sessionId: '', project: PROJECT, routine: run }], active: 0 }));
    const { restoreWorkspace } = await relaunch();
    const after = storeWithMock();

    expect(await restoreWorkspace(after.store)).toBe(false);

    expect(after.of('agent_start')).toHaveLength(0);
    expect(after.store.sessions).toHaveLength(0);
  });
});
