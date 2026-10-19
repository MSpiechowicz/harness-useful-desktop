import type { RoutineRun } from '../bindings/RoutineRun';
import type { RoutineRunsPage } from '../bindings/RoutineRunsPage';
import type { RoutineRunsQuery } from '../bindings/RoutineRunsQuery';
import type { RoutinesEvent } from '../bindings/RoutinesEvent';
import type { RunStatus } from '../bindings/RunStatus';
import { IpcFailure } from '../transport';
import { liveRun, type RunSource } from './routine-fixtures';

/** How long a started run goes on before the mock ends it. */
const RUN_MS = 2500;

function now(): string {
  return new Date().toISOString().replace(/\.\d+Z$/, 'Z');
}

/** The runs of the mock backend: the history of the finished ones, newest first, and the ones going on now. */
export class MockRuns {
  private history: RoutineRun[] = [];
  private live = new Map<string, RoutineRun>();

  constructor(private readonly emit: (event: RoutinesEvent) => void) {}

  /** Starts with these runs: `finished` newest first, and `going` on. */
  seed(finished: RoutineRun[], going: RoutineRun[]): void {
    this.history = finished;

    for (const run of going) {
      this.live.set(run.id, run);
    }
  }

  /** Whether a run of the routine is going on. */
  isRunning(routineId: string): boolean {
    return [...this.live.values()].some((run) => run.routineId === routineId);
  }

  /** Every run, the ones going on first. */
  all(): RoutineRun[] {
    return [...this.live.values(), ...this.history];
  }

  /** Starts a manual run of a routine. It ends by itself a moment later. */
  start(source: RunSource): RoutineRun {
    const run: RoutineRun = { ...liveRun(source, false, 0), id: `f${Date.now().toString(16).padStart(31, '0')}`, trigger: 'manual' };

    this.live.set(run.id, run);
    this.emit({ kind: 'runStarted', run });
    setTimeout(() => this.finish(run.id, 'ok'), RUN_MS);

    return run;
  }

  /** The user answered what a run asked: it goes on without waiting, and the listener hears that something changed. */
  answer(runId: string): void {
    const run = this.live.get(runId);

    if (run?.needsYou) {
      this.live.set(runId, { ...run, needsYou: false });
      this.emit({ kind: 'changed' });
    }
  }

  /** Ends a live run: it moves to the history with the status, and the listener hears of it. */
  private finish(runId: string, status: RunStatus): void {
    const run = this.live.get(runId);

    if (!run) {
      return;
    }

    const durationMs = Date.now() - Date.parse(run.startedAt);
    const done: RoutineRun = { ...run, status, needsYou: false, endedAt: now(), durationMs, message: status === 'stopped' ? 'Stopped by you.' : null, excerpt: run.session ? 'Done. Nothing needs you.' : 'ok' };

    this.live.delete(runId);
    this.history = [done, ...this.history];
    this.emit({ kind: 'runFinished', run: done });
  }

  stop(runId: string): null {
    if (!this.live.has(runId)) {
      throw IpcFailure.from({ code: 'notFound', message: 'this run is not running' });
    }

    this.finish(runId, 'stopped');

    return null;
  }

  /** One page of the runs, newest first: the live ones lead. */
  page(query: RoutineRunsQuery): RoutineRunsPage {
    const matching = this.all().filter((run) => (query.routineId === null || run.routineId === query.routineId) && (query.status === null || run.status === query.status));
    const runs = matching.slice(query.offset, query.offset + query.limit);
    const end = query.offset + runs.length;

    return { runs, total: matching.length, nextOffset: end < matching.length ? end : null };
  }

  /** Forgets the runs of a deleted routine. */
  dropRunsOf(routineId: string): void {
    this.history = this.history.filter((run) => run.routineId !== routineId);

    for (const run of [...this.live.values()].filter((candidate) => candidate.routineId === routineId)) {
      this.live.delete(run.id);
    }
  }
}
