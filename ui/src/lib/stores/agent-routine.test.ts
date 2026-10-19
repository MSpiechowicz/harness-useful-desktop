import { describe, expect, it, vi } from 'vitest';

import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import type { SessionRoutineTag } from '$lib/ipc/bindings/SessionRoutineTag';

import { PROJECT, storeWithMock } from '../../../tests/chat-helpers';

/** The run of the sample routines that goes on and asks the user something: the chat a notification points at. */
async function askingRun(backend: ReturnType<typeof storeWithMock>): Promise<{ run: RoutineRun; tag: SessionRoutineTag }> {
  const page = await backend.transport.invoke('routine_runs', { query: { routineId: null, status: 'running', offset: 0, limit: 20 } });
  const run = page.runs.find((candidate) => candidate.needsYou) as RoutineRun;

  return { run, tag: { runId: run.id, routineId: run.routineId, name: run.routineName } };
}

describe('the chat tab of a routine run', () => {
  it('opens as a normal tab, shows the replayed conversation and the question, and wears the routine', async () => {
    const backend = storeWithMock();
    const { tag } = await askingRun(backend);

    const session = await backend.store.attachRoutine(tag);

    expect(session?.routine).toEqual(tag);
    expect(backend.store.activeKey).toBe(session?.key);
    expect(backend.of('routine_run_attach')).toHaveLength(1);
    expect(backend.of('agent_start')).toHaveLength(0);
    await vi.waitFor(() => expect(session?.chat.openRequests).toHaveLength(1), { timeout: 5000 });
  });

  it('answers the question of the run like any chat, addressing the run session', async () => {
    const backend = storeWithMock();
    const { tag } = await askingRun(backend);
    const session = (await backend.store.attachRoutine(tag))!;
    await vi.waitFor(() => expect(session.chat.openRequests).toHaveLength(1), { timeout: 5000 });
    const request = session.chat.openRequests[0];

    await session.respond(request.id, { type: 'confirmed', confirmed: true });

    expect(session.chat.openRequests).toHaveLength(0);
    expect(backend.of('agent_ui_respond')).toEqual([{ key: session.key, response: { id: request.id, outcome: { type: 'confirmed', confirmed: true } } }]);
  });

  it('shows the tab that is open instead of attaching a second window to the same run', async () => {
    const backend = storeWithMock();
    const { tag } = await askingRun(backend);
    const first = await backend.store.attachRoutine(tag);
    const other = await backend.store.create('omp', PROJECT);

    expect(backend.store.activeKey).toBe(other?.key);

    const again = await backend.store.attachRoutine(tag);

    expect(again).toBe(first);
    expect(backend.store.activeKey).toBe(first?.key);
    expect(backend.of('routine_run_attach')).toHaveLength(1);
  });

  it('opens one tab when two things ask for the same run at once, a toast action and a notification click', async () => {
    const backend = storeWithMock();
    const { tag } = await askingRun(backend);

    const [first, second] = await Promise.all([backend.store.attachRoutine(tag), backend.store.attachRoutine(tag, { activate: false })]);

    expect(first).not.toBeNull();
    expect(second).toBe(first);
    expect(backend.store.sessions).toHaveLength(1);
    expect(backend.store.activeKey).toBe(first?.key);
    expect(backend.of('routine_run_attach')).toHaveLength(1);
  });

  it('attaches again after a failed attach, when the run is asked for later', async () => {
    const backend = storeWithMock();
    const gone = { runId: 'f'.repeat(32), routineId: 'c'.repeat(32), name: 'Gone' };

    const [first, second] = await Promise.all([backend.store.attachRoutine(gone, { quiet: true }), backend.store.attachRoutine(gone, { quiet: true })]);
    await backend.store.attachRoutine(gone, { quiet: true });

    expect([first, second]).toEqual([null, null]);
    expect(backend.of('routine_run_attach')).toHaveLength(2);
  });

  it('is stopped like any chat when its tab is closed', async () => {
    const backend = storeWithMock();
    const { tag } = await askingRun(backend);
    const session = (await backend.store.attachRoutine(tag))!;

    await backend.store.stop(session.key);

    expect(backend.of('agent_stop')).toEqual([{ key: session.key }]);
    expect(backend.store.sessions).toHaveLength(0);
  });

  it('says why when the run cannot be opened, and opens no tab', async () => {
    const backend = storeWithMock();

    const session = await backend.store.attachRoutine({ runId: 'f'.repeat(32), routineId: 'c'.repeat(32), name: 'Gone' });

    expect(session).toBeNull();
    expect(backend.store.sessions).toHaveLength(0);
    expect(backend.notifications.items.at(-1)).toMatchObject({ title: 'Could not open the chat of the run', tone: 'danger' });
  });

  it('opens a run that ended quietly, for the caller to fall back to the saved session', async () => {
    const backend = storeWithMock();

    await backend.store.attachRoutine({ runId: 'f'.repeat(32), routineId: 'c'.repeat(32), name: 'Gone' }, { quiet: true });

    expect(backend.notifications.items).toHaveLength(0);
  });
});
