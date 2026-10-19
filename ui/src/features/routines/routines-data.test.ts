import { waitFor } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { ROUTINES_WORKSPACE } from '$lib/ipc/mock/routine-fixtures';

import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import type { RoutinesEvent } from '$lib/ipc/bindings/RoutinesEvent';
import { toasts } from '$lib/stores/toasts.svelte';

import { routines } from './routines-data.svelte';
import { fakeRoutines, fakeRoutinesWhere, PROJECT, registerRoutinesHooks } from './routines-test-support';

registerRoutinesHooks();

/** Starts the store listening and answers the events a backend would send, as the backend would. */
async function listen() {
  const { state, invoke } = fakeRoutines();
  routines.follow(PROJECT);
  await waitFor(() => expect(routines.listing).not.toBeNull());
  const stop = routines.start();
  const heard: RoutinesEvent[] = [];
  const unsubscribe = routines.subscribe((event) => heard.push(event));

  await waitFor(() => expect(routines.running.size).toBeGreaterThan(0));

  return { state, invoke, heard, stop: () => (stop(), unsubscribe()) };
}

describe('The routines store', () => {
  it('counts what waits for the user: the routines to approve and the runs that ask something', async () => {
    const { stop } = await listen();

    expect(routines.pending).toBe(2);
    expect([...routines.waiting.keys()]).toHaveLength(1);
    expect(routines.attention).toBe(3);
    stop();
  });

  it('reads again when the backend says something changed, and tells whoever subscribed', async () => {
    const { state, heard, stop } = await listen();

    state.agentAdds('Written by an agent');

    await waitFor(() => expect(routines.views.map((view) => view.name)).toContain('Written by an agent'));
    expect(heard).toContainEqual({ kind: 'changed' });
    expect(routines.pending).toBe(3);
    stop();
  });

  it('knows a run going on from its start to its end, and raises the revision the history reads again on', async () => {
    const { state, heard, stop } = await listen();
    const idle = routines.views.find((view) => view.status === 'active') as NonNullable<(typeof routines.views)[number]>;
    const before = routines.revision;

    state.runNow(idle.id);

    await waitFor(() => expect(routines.runOf(idle.id)).not.toBeNull());
    expect(routines.revision).toBeGreaterThan(before);
    expect(heard.some((event) => event.kind === 'runStarted')).toBe(true);

    state.stop(routines.runOf(idle.id)?.id ?? '');

    await waitFor(() => expect(routines.runOf(idle.id)).toBeNull());
    expect(heard.some((event) => event.kind === 'runFinished')).toBe(true);
    stop();
  });

  it('toasts a failed run while the window is in front, with the routine\'s name', async () => {
    const { stop } = await listen();
    const run = routines.running.values().next().value as RoutineRun;

    (routines as unknown as { handle: (event: RoutinesEvent) => void }).handle({ kind: 'runFinished', run: { ...run, status: 'failed', endedAt: '2026-10-18T09:03:12Z', durationMs: 1000 } });

    expect(toasts.items.at(-1)).toMatchObject({ title: 'Routine failed', tone: 'danger' });
    stop();
  });

  it('remembers that the backend asked to show a run, for the shell to open', async () => {
    const { stop } = await listen();

    (routines as unknown as { handle: (event: RoutinesEvent) => void }).handle({ kind: 'open', runId: 'c'.repeat(32) });

    expect(routines.openRequest).toBe('c'.repeat(32));
    stop();
  });

  it.each(['changed', 'runFinished'] as const)('looks for the folder of the global routines again on %s while it does not know it, so Sessions offers it after the first global run', async (kind) => {
    const { stop } = await listen();
    routines.workspace = null;
    const run = routines.running.values().next().value as RoutineRun;

    emit(kind === 'changed' ? { kind } : { kind, run: { ...run, status: 'ok', endedAt: '2026-10-18T09:03:12Z', durationMs: 1000 } });

    await waitFor(() => expect(routines.workspace).toBe(ROUTINES_WORKSPACE));
    stop();
  });
});

/** The toast of the run that asks the user, as the shell shows it. */
const questionToasts = () => toasts.items.filter((toast) => toast.title === 'Routine needs you');

function emit(event: RoutinesEvent): void {
  (routines as unknown as { handle: (event: unknown) => void }).handle(event);
}

describe('A run that stops waiting for the user', () => {
  function asking(): RoutineRun {
    return [...routines.running.values()].find((run) => run.needsYou) as RoutineRun;
  }

  it('takes its toast down and its count off the rail when the runs read again say it does not ask any more', async () => {
    const { state, stop } = await listen();
    const run = asking();
    emit({ kind: 'needsYou', runId: run.id, routineId: run.routineId, name: run.routineName });

    expect(questionToasts()).toHaveLength(1);
    expect(questionToasts()[0].duration).toBe(0);
    await waitFor(() => expect(routines.waiting.size).toBe(1));
    expect(routines.attention).toBe(3);

    state.answer(run.id);

    await waitFor(() => expect(routines.waiting.size).toBe(0));
    expect(questionToasts()).toHaveLength(0);
    expect(routines.attention).toBe(2);
    stop();
  });

  it('keeps the toast of a question while the runs read say the run still asks', async () => {
    const { stop } = await listen();
    const run = asking();

    emit({ kind: 'needsYou', runId: run.id, routineId: run.routineId, name: run.routineName });
    await routines.refresh();

    expect(questionToasts()).toHaveLength(1);
    expect(routines.waiting.size).toBe(1);
    stop();
  });

  it('is cleared at once when the backend says so, before the runs are read again', async () => {
    const { state, stop } = await listen();
    const run = asking();
    emit({ kind: 'needsYou', runId: run.id, routineId: run.routineId, name: run.routineName });
    state.answer(run.id);

    emit({ kind: 'needsYouCleared', runId: run.id, routineId: run.routineId });

    expect(routines.waiting.size).toBe(0);
    expect(routines.attention).toBe(2);
    expect(questionToasts()).toHaveLength(0);
    stop();
  });

  it('takes the toast down when the run ends', async () => {
    const { stop } = await listen();
    const run = asking();
    emit({ kind: 'needsYou', runId: run.id, routineId: run.routineId, name: run.routineName });

    emit({ kind: 'runFinished', run: { ...run, needsYou: false, status: 'stopped', endedAt: '2026-10-18T09:03:12Z', durationMs: 1000 } });

    expect(questionToasts()).toHaveLength(0);
    stop();
  });
});

describe('A fresh start of the store', () => {
  it('drops the answers to reads that started before it', async () => {
    const answers: (() => void)[] = [];
    const later = <T>(value: T) => new Promise<T>((resolve) => answers.push(() => resolve(value)));
    const run = { id: 'run-before', routineId: 'r1', routineName: 'Before', status: 'running', needsYou: true } as unknown as RoutineRun;
    fakeRoutinesWhere((view) => view, {
      routine_runs: () => later({ runs: [run], total: 1, nextOffset: null }),
      routines_workspace: () => later(ROUTINES_WORKSPACE),
    });

    routines.follow(PROJECT);
    void routines.readWorkspace();
    await waitFor(() => expect(answers).toHaveLength(2));
    routines.reset();

    for (const answer of answers) {
      answer();
    }
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(routines.running.size).toBe(0);
    expect(routines.waiting.size).toBe(0);
    expect(routines.workspace).toBeNull();
  });
});
