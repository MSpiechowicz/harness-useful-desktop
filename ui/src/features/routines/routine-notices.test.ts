import { describe, expect, it, vi } from 'vitest';

import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { RunNotices } from './routine-notices';

const RUN: RoutineRun = {
  routineId: 'a'.repeat(32),
  needsYou: false,
  id: 'b'.repeat(32),
  routineName: 'Morning review',
  origin: 'local',
  trigger: 'schedule',
  startedAt: '2026-10-18T09:00:00Z',
  endedAt: '2026-10-18T09:03:12Z',
  durationMs: 192_000,
  status: 'ok',
  message: null,
  excerpt: null,
  errorExcerpt: null,
  exitCode: null,
  session: null,
  project: null,
};

function finished(patch: Partial<RoutineRun>) {
  return { kind: 'runFinished' as const, run: { ...RUN, ...patch } };
}

describe('the toasts of a run, while the window is in front', () => {
  it('always says a run failed or needs attention, with the way to see it', () => {
    const notices = new RunNotices(() => true);

    expect(notices.toastFor(finished({ status: 'failed' }))).toMatchObject({ title: 'Routine failed', description: 'Morning review · See the run in Harness Desktop', tone: 'danger' });
    expect(notices.toastFor(finished({ status: 'failed' }))).not.toBeNull();
    expect(notices.toastFor(finished({ status: 'needsAttention' }))).toMatchObject({ title: 'Routine needs attention', tone: 'warning' });
  });

  it('says a run that went well at most once an hour for each routine', () => {
    let at = Date.parse('2026-10-18T09:00:00Z');
    const notices = new RunNotices(() => true, () => at);

    expect(notices.toastFor(finished({}))).toMatchObject({ title: 'Routine finished', description: 'Morning review', tone: 'success' });

    at += 59 * 60_000;
    expect(notices.toastFor(finished({}))).toBeNull();
    expect(notices.toastFor(finished({ routineId: 'c'.repeat(32) }))).not.toBeNull();

    at += 2 * 60_000;
    expect(notices.toastFor(finished({}))).not.toBeNull();
  });

  it('says nothing of a run that was stopped or is still going', () => {
    const notices = new RunNotices(() => true);

    expect(notices.toastFor(finished({ status: 'stopped' }))).toBeNull();
    expect(notices.toastFor({ kind: 'runStarted', run: { ...RUN, status: 'running' } })).toBeNull();
    expect(notices.toastFor({ kind: 'changed' })).toBeNull();
  });

  it('keeps a question that needs the user until it is dismissed, and offers to open its chat', () => {
    const notices = new RunNotices(() => true);
    const toast = notices.toastFor({ kind: 'needsYou', runId: RUN.id, routineId: RUN.routineId, name: 'Morning review' });

    expect(toast).toMatchObject({ title: 'Routine needs you', description: 'Morning review · Answer it in Harness Desktop', tone: 'warning', duration: 0 });
    expect(toast?.action?.label).toBe('Open chat');
  });

  it('opens the run history on the run from a failure toast', () => {
    const toast = new RunNotices(() => true).toastFor(finished({ status: 'failed' }));

    toast?.action?.run();

    expect(viewState.routinesSection).toBe('runs');
    expect(viewState.runId).toBe(RUN.id);
    expect(viewState.runsRoutine).toBe(RUN.routineId);
    viewState.reset('routines');
    nav.go('chat');
  });

  it('leaves everything to the native notification while the window is not in front', () => {
    const focused = vi.fn(() => false);
    const notices = new RunNotices(focused);

    expect(notices.toastFor(finished({ status: 'failed' }))).toBeNull();
    expect(notices.toastFor({ kind: 'needsYou', runId: RUN.id, routineId: RUN.routineId, name: 'Morning review' })).toBeNull();
    expect(focused).toHaveBeenCalled();
  });
});
