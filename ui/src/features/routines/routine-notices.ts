import { t } from '$lib/i18n/i18n.svelte';
import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import type { RoutinesEvent } from '$lib/ipc/bindings/RoutinesEvent';
import { nav } from '$lib/stores/nav.svelte';
import type { ToastInput } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { spelledOut } from '../memory/script-format';
import { openRunChat } from './routine-open';

/** How often a routine that went well says so: at most once an hour, as the native notification does. Failures and questions are always said. */
const FINISHED_EVERY_MS = 60 * 60_000;

/** Opens the run history on a run, from a toast. */
export function showRun(run: Pick<RoutineRun, 'id' | 'routineId'>): void {
  viewState.routinesSection = 'runs';
  viewState.runsRoutine = run.routineId;
  viewState.runsStatus = 'all';
  viewState.runsPage = 0;
  viewState.runId = run.id;
  nav.go('routines');
}

/** The run an event says stopped waiting for the user (answered, cancelled, ended), or null. */
export function stoppedWaiting(event: RoutinesEvent): string | null {
  return event.kind === 'needsYouCleared' ? event.runId : null;
}

/**
 * The toasts the events of the backend ask for. The backend shows a native notification for a run that fails, needs attention or
 * waits for the user, and for one that went well at most once an hour, but not while the window is in front: then the app says it
 * itself, in the same words and on the same terms.
 */
export class RunNotices {
  private finished = new Map<string, number>();

  /** The question toasts on screen: the toast of each run and how many reads of the runs had started when it came up. */
  private asking = new Map<string, { toast: number; after: number }>();

  /** Remembers the toast that tells a run needs the user, to take it down when the run stops asking. */
  track(runId: string, toast: number, after: number): void {
    this.asking.set(runId, { toast, after });
  }

  /** Forgets the question toast of a run and returns it, or null when none is on screen. */
  release(runId: string): number | null {
    const entry = this.asking.get(runId);
    this.asking.delete(runId);

    return entry?.toast ?? null;
  }

  /** The question toasts of runs not in `asking` according to read number `read` (one that started after the toast came up), forgotten here. */
  answered(asking: ReadonlySet<string>, read: number): number[] {
    const done = [...this.asking].filter(([runId, entry]) => read > entry.after && !asking.has(runId));

    return done.map(([runId]) => this.release(runId)).filter((toast): toast is number => toast !== null);
  }

  constructor(
    /** Whether the window is in front. */
    public focused: () => boolean = () => document.hasFocus(),
    private now: () => number = Date.now,
  ) {}

  /** The toast an event asks for, or null. */
  toastFor(event: RoutinesEvent): ToastInput | null {
    if (!this.focused()) {
      return null;
    }

    if (event.kind === 'needsYou') {
      return {
        title: t('routines.notice.needsYou'),
        description: t('routines.notice.needsYou.body', { name: spelledOut(event.name) }),
        tone: 'warning',
        duration: 0,
        action: { label: t('routines.panel.openChat'), run: () => void openRunChat({ id: event.runId, routineId: event.routineId, routineName: event.name, session: null, project: null }) },
      };
    }

    return event.kind === 'runFinished' ? this.finishedToast(event.run) : null;
  }

  private finishedToast(run: RoutineRun): ToastInput | null {
    const name = spelledOut(run.routineName);
    const action = { label: t('routines.notice.showRun'), run: () => showRun(run) };

    switch (run.status) {
      case 'failed':
        return { title: t('routines.notice.failed'), description: t('routines.notice.seeRun', { name }), tone: 'danger', action };
      case 'needsAttention':
        return { title: t('routines.notice.needsAttention'), description: t('routines.notice.seeRun', { name }), tone: 'warning', action };
      case 'ok':
        return this.sayFinished(run, name, action);
      default:
        return null;
    }
  }

  private sayFinished(run: RoutineRun, name: string, action: NonNullable<ToastInput['action']>): ToastInput | null {
    const last = this.finished.get(run.routineId);
    const at = this.now();

    if (last !== undefined && at - last < FINISHED_EVERY_MS) {
      return null;
    }

    this.finished.set(run.routineId, at);

    return { title: t('routines.notice.finished'), description: name, tone: 'success', action };
  }
}
