import { t, tn } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import type { RoutineDraft } from '$lib/ipc/bindings/RoutineDraft';
import type { RoutineListing } from '$lib/ipc/bindings/RoutineListing';
import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import type { RoutineRunsPage } from '$lib/ipc/bindings/RoutineRunsPage';
import type { RoutineRunsQuery } from '$lib/ipc/bindings/RoutineRunsQuery';
import type { RoutinesEvent } from '$lib/ipc/bindings/RoutinesEvent';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import type { RoutineWhen } from '$lib/ipc/bindings/RoutineWhen';
import type { SchedulePreview } from '$lib/ipc/bindings/SchedulePreview';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { DocBusy } from '$lib/stores/doc-busy.svelte';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { ViewLoader } from '$lib/stores/view-loader.svelte';

import { RunNotices, stoppedWaiting } from './routine-notices';
import { originalOf } from './routine-team';
import { isQueued, pendingCount, routineTitle, viewKey, waitingRoutines, whenText } from './routine-format';
import { toggleDraft } from './routine-form';

/** The routines list has one reading per project, so the loader's key is a constant. */
const KEY = 'routines';

/** How many runs one page of the run history holds. */
export const RUNS_PAGE_SIZE = 50;

/** The most runs that can be going on at once (2 agent runs and 4 command or script runs, with a few waiting), asked for in one read. */
const RUNNING_LIMIT = 50;

/** The run history asked for, as the loader's key: it fetches again whenever any of it changes, and drops answers to older ones. */
export function runsKey(query: RoutineRunsQuery): string {
  return JSON.stringify(query);
}

/**
 * The routines of the active project (and the global ones), read once for the whole app so the rail can count the ones
 * waiting for the user while another view is open. The backend tells when the folder, the approvals or a run change, and
 * the list is read again then. The one `routines_listen` listener of the app is this store's: whoever else wants the events
 * (a toast, a navigation) subscribes here.
 */
export class RoutinesStore {
  readonly list = new ViewLoader<RoutineListing, string>((_key, path) => ipc.invoke('routines_list', { project: path }));

  /** One page of the run history, for the query it is keyed by (`runsKey`). */
  readonly runs = new ViewLoader<RoutineRunsPage, string>((key) => ipc.invoke('routine_runs', { query: JSON.parse(key) as RoutineRunsQuery }));

  private readonly work = new DocBusy();

  /** The keys (`viewKey`) of routines a switch or a button is working on, so a second click waits. */
  get busy(): ReadonlySet<string> {
    return this.work.keys;
  }

  /** The runs going on now, by run id. */
  running = $state.raw<ReadonlyMap<string, RoutineRun>>(new Map());

  /** The folder global routines run in, where their sessions are kept; null until one has run. */
  workspace = $state<string | null>(null);

  /** Whether a folder is the routines' folder or inside it: each global routine works in a folder of its own below it. */
  inWorkspace(path: string): boolean {
    const folder = this.workspace;

    return folder !== null && (path === folder || path.startsWith(`${folder}/`) || path.startsWith(`${folder}\\`));
  }

  /** Raised on every change of the runs, so a view that shows them reads again. */
  revision = $state(0);

  /** A new routine was asked for from outside the view (the command palette): the view opens the editor and clears it. */
  wantsNew = $state(false);

  /** The run the backend asked to show (a click on a "needs you" notification): the shell attaches to it and clears it. */
  openRequest = $state<string | null>(null);

  /** What the events say as toasts while the window is in front. */
  readonly notices = new RunNotices();

  private listeners = new Set<(event: RoutinesEvent) => void>();

  /** How many reads of the runs going on have started, so that an answer to one that started before a change cannot undo it. */
  private reads = 0;
  /** The reads that started up to this one are out of date: a run stopped waiting after they started. */
  private stale = 0;
  /** The newest read whose answer is shown. */
  private shown = 0;
  /** Raised by `reset`, so an answer to a read that started before it is dropped instead of filling the store again. */
  private epoch = 0;

  get listing(): RoutineListing | null {
    return this.list.data;
  }

  get views(): RoutineView[] {
    return this.list.data?.routines ?? [];
  }

  /** How many routines wait for the user's approval. */
  get pending(): number {
    return pendingCount(this.list.data);
  }

  /** The routines with a run that asks the user something, by routine id, with that run's id. */
  get waiting(): ReadonlyMap<string, string> {
    return waitingRoutines(this.running.values());
  }

  /** How many routines need the user: those waiting for approval and those with a run that asks something. */
  get attention(): number {
    return this.pending + this.waiting.size;
  }

  /** Reads the routines of this project (or the global ones, with none) and the runs going on; a list already read is refreshed quietly. */
  follow(path: string | null): void {
    this.list.revalidate(KEY, path);
    void this.readRunning();
  }

  /**
   * Asks the team database for the team routines of this project now (it can take seconds), then reads the list again. The list
   * is shown first with the copies already there, so the view never waits for the network. A failure shows as the team notice.
   */
  async refreshTeam(path: string | null): Promise<void> {
    try {
      await ipc.invoke('routines_team_refresh', { project: path });
    } catch {
      // The listing says why the team routines are not read, when they are not.
    }

    await this.list.refresh({ quiet: true });
  }

  /** Listens for changes of the routines and their runs, and reads again on each. Returns the cleanup. */
  start(): () => void {
    let listening = true;

    ipc.invoke('routines_listen', {
      onEvent: (event) => {
        if (listening) {
          this.handle(event);
        }
      },
    })
      .then(() => Promise.all([this.readRunning(), this.readWorkspace()]))
      .catch(() => {});

    return () => {
      listening = false;
    };
  }

  /** Tells `listener` of every event of the backend until the returned function is called. */
  subscribe(listener: (event: RoutinesEvent) => void): () => void {
    this.listeners.add(listener);

    return () => this.listeners.delete(listener);
  }

  private handle(event: RoutinesEvent): void {
    if (event.kind === 'open') {
      this.openRequest = event.runId;
    }

    if (event.kind === 'runStarted' || event.kind === 'runFinished' || event.kind === 'needsYou') {
      this.revision += 1;
    }

    // The first global run creates its folder, which can be told by any of its events.
    if (this.workspace === null && (event.kind === 'runStarted' || event.kind === 'runFinished' || event.kind === 'changed')) {
      void this.readWorkspace();
    }

    const answered = event.kind === 'runFinished' ? event.run.id : stoppedWaiting(event);

    if (answered !== null) {
      this.stopWaiting(answered);
    }

    const toast = this.notices.toastFor(event);

    if (toast && event.kind === 'needsYou') {
      this.notices.track(event.runId, toasts.push(toast), this.reads);
    } else if (toast) {
      toasts.push(toast);
    }

    void this.refresh();

    for (const listener of this.listeners) {
      listener(event);
    }
  }

  /** A run does not wait for the user any more (answered, cancelled, ended): its state, its toast and the rail's count follow at once. */
  private stopWaiting(runId: string): void {
    const run = this.running.get(runId);

    if (run?.needsYou) {
      this.running = new Map(this.running).set(runId, { ...run, needsYou: false });
    }

    this.stale = this.reads;
    this.revision += 1;

    const toast = this.notices.release(runId);

    if (toast !== null) {
      toasts.dismiss(toast);
    }
  }

  /** Reads the list, the runs going on and the run history on screen again in the background. */
  async refresh(): Promise<void> {
    await Promise.all([this.list.refresh({ quiet: true }), this.readRunning()]);
    this.revision += 1;
  }

  /** Reads the folder global routines run in, once one has run (it is null until then). */
  async readWorkspace(): Promise<void> {
    const epoch = this.epoch;

    try {
      const folder = await ipc.invoke('routines_workspace', undefined);

      if (epoch === this.epoch) {
        this.workspace = folder;
      }
    } catch {
      // The folder is asked for again with the next first run.
    }
  }

  private async readRunning(): Promise<void> {
    this.reads += 1;
    const started = this.reads;
    const epoch = this.epoch;

    try {
      const page = await ipc.invoke('routine_runs', { query: { routineId: null, status: 'running', offset: 0, limit: RUNNING_LIMIT } });

      // An answer to a read that started before a reset, a change, or a newer read's, is older than what is shown.
      if (epoch !== this.epoch || started <= this.stale || started < this.shown) {
        return;
      }

      this.shown = started;
      this.running = new Map(page.runs.map((run) => [run.id, run]));

      // A question toast stays only while its run still asks, as far as a read that started after the toast knows.
      const asking = new Set(page.runs.filter((run) => run.needsYou).map((run) => run.id));

      for (const toast of this.notices.answered(asking, started)) {
        toasts.dismiss(toast);
      }
    } catch {
      // The runs going on are read again with the next change.
    }
  }

  /** The routine with this row key (`viewKey`). */
  find(key: string | null): RoutineView | null {
    return key === null ? null : (this.views.find((view) => viewKey(view) === key) ?? null);
  }

  /** The run going on now for a routine, if any. */
  runOf(routineId: string): RoutineRun | null {
    return [...this.running.values()].find((run) => run.routineId === routineId) ?? null;
  }

  /** Creates or edits a routine (the written version is approved at once). Throws what the backend says, for the editor to show. */
  async save(draft: RoutineDraft): Promise<RoutineView> {
    try {
      return await ipc.invoke('routine_save', { draft });
    } finally {
      await this.refresh();
    }
  }

  /** The next runs of a schedule, or why it is not valid. Throws when the backend cannot say. */
  preview(when: RoutineWhen): Promise<SchedulePreview> {
    return ipc.invoke('routine_schedule_preview', { when });
  }

  /** Turns a routine's schedule on or off, keeping everything else as it is. A team routine's switch is a choice of this computer only. */
  async toggle(view: RoutineView, enabled: boolean): Promise<boolean> {
    if (view.origin === 'team') {
      return this.run(viewKey(view), 'routines.toast.toggleFailed', () => ipc.invoke('routines_team_enable', { id: view.id, enabled, project: view.teamRepo === null ? null : (project.active?.path ?? null) }));
    }

    const routine = view.routine;

    if (!routine) {
      return false;
    }

    return this.run(viewKey(view), 'routines.toast.toggleFailed', () => ipc.invoke('routine_save', { draft: toggleDraft(view, routine, enabled) }));
  }

  /** Approves exactly the version on screen. */
  async approve(view: RoutineView): Promise<boolean> {
    const done = await this.run(viewKey(view), 'routines.toast.approveFailed', () => ipc.invoke('routine_approve', { id: view.id, sha256: view.sha256 }));

    if (done && view.routine) {
      toasts.push({ title: t('routines.toast.approved', { name: routineTitle(view) }), description: t('routines.panel.status.active', { when: whenText(view.routine.when) }), tone: 'success' });
    }

    return done;
  }

  /** Puts an edited routine back as the user approved it, and removes one that was never approved. */
  async reject(view: RoutineView): Promise<boolean> {
    const restores = view.approval === 'changed';
    const done = await this.run(viewKey(view), 'routines.toast.rejectFailed', () => ipc.invoke('routine_reject', { id: view.id, sha256: view.sha256 }));

    if (done) {
      toasts.push({ title: t(restores ? 'routines.toast.rejected.restored' : 'routines.toast.rejected.removed'), tone: 'success' });
    }

    return done;
  }

  async remove(view: RoutineView): Promise<boolean> {
    const done = await this.run(viewKey(view), 'routines.toast.deleteFailed', () => ipc.invoke('routine_delete', { id: view.id, sha256: view.sha256 }));

    if (done) {
      toasts.push({ title: t('routines.toast.deleted', { name: routineTitle(view) }), tone: 'success' });
    }

    return done;
  }

  /** Starts a run now. The run shows in the history at once, unless all the places for runs are taken: then it waits for one. */
  async runNow(view: RoutineView): Promise<boolean> {
    // A run that waits starts by itself: asking again is not a conflict of the file, so it does not say the routine changed.
    if (isQueued(view)) {
      this.sayQueued(view);

      return false;
    }

    let started: RoutineRun | null = null;
    let waiting = false;
    const done = await this.run(viewKey(view), 'routines.toast.runFailed', async () => {
      try {
        started = await ipc.invoke('routine_run_now', { id: view.id });
      } catch (error) {
        // The routine started waiting since the list was read: the backend refuses with a conflict, and the list says why.
        if (IpcFailure.from(error).code !== 'conflict') {
          throw error;
        }

        await this.refresh();
        waiting = isQueued(this.find(viewKey(view)) ?? view);

        if (!waiting) {
          throw error;
        }
      }
    });

    if (waiting) {
      this.sayQueued(view);

      return false;
    }

    if (done) {
      toasts.push({ title: t(started === null ? 'routines.toast.queued' : 'routines.toast.started', { name: routineTitle(view) }), tone: 'success' });
    }

    return done;
  }

  private sayQueued(view: RoutineView): void {
    toasts.push({ title: t('routines.toast.alreadyQueued'), description: routineTitle(view), tone: 'info' });
  }

  /** Stops a run that is going on. */
  async stop(run: RoutineRun): Promise<boolean> {
    return this.run(run.id, 'routines.toast.stopFailed', () => ipc.invoke('routine_run_stop', { runId: run.id }));
  }

  /** Shares a local routine with the team, as the version on screen. */
  async share(view: RoutineView): Promise<boolean> {
    const done = await this.run(viewKey(view), 'routines.toast.shareFailed', () => ipc.invoke('routines_share', { id: view.id, sha256: view.sha256 }));

    if (done) {
      toasts.push({ title: t('routines.toast.shared', { name: routineTitle(view) }), tone: 'success' });
    }

    return done;
  }

  /** Puts the local original's current version on the team in place of the team copy. Does nothing when the original is gone. */
  async updateTeam(team: RoutineView): Promise<boolean> {
    const original = originalOf(this.views, team);

    if (!original) {
      return false;
    }

    const done = await this.run(viewKey(team), 'routines.toast.teamUpdateFailed', () => ipc.invoke('routines_share', { id: original.id, sha256: original.sha256 }));

    if (done) {
      toasts.push({ title: t('routines.toast.teamUpdated', { name: routineTitle(original) }), tone: 'success' });
    }

    return done;
  }

  /** Takes a team routine off the team, for every teammate. */
  async removeTeam(team: RoutineView): Promise<boolean> {
    const done = await this.run(viewKey(team), 'routines.toast.teamRemoveFailed', () => ipc.invoke('routines_team_remove', { id: team.id, sha256: team.sha256 }));

    if (done) {
      toasts.push({ title: t('routines.toast.teamRemoved', { name: routineTitle(team) }), tone: 'success' });
    }

    return done;
  }

  /** Copies a team routine to the local routines, where it is the user's own to edit. Answers the local routine, or null when it failed. */
  async copyTeam(team: RoutineView, path: string | null): Promise<RoutineView | null> {
    let copy: RoutineView | null = null;
    const done = await this.run(viewKey(team), 'routines.toast.copyFailed', async () => {
      copy = await ipc.invoke('routines_team_copy', { id: team.id, sha256: team.sha256, project: path });
    });

    if (done) {
      toasts.push({ title: t('routines.toast.copied', { name: routineTitle(team) }), tone: 'success' });
    }

    return copy;
  }

  /** Deletes every routine of the global scope and of the project, then says how many went. Throws what the backend says, for the dialog to show. */
  async removeAll(projectPath: string | null): Promise<number> {
    try {
      const { deleted } = await ipc.invoke('routines_delete_all', { project: projectPath });

      toasts.push({ title: tn('routines.deleteAll.done', deleted), tone: 'success' });

      return deleted;
    } finally {
      await this.refresh();
    }
  }

  /** Removes every routine this user shared with the team, then says how many went. Throws what the backend says, for the dialog to show. */
  async removeMine(): Promise<number> {
    try {
      const { deleted } = await ipc.invoke('routines_team_remove_mine', undefined);

      toasts.push({ title: tn('routines.removeMine.done', deleted), tone: 'success' });

      return deleted;
    } finally {
      await this.refresh();
    }
  }

  /** Runs one change on a routine: marks it busy, then reads the list again whatever happened. */
  private run(key: string, failed: MessageKey, change: () => Promise<unknown>): Promise<boolean> {
    return this.work.run(key, change, { failed, conflict: 'routines.toast.conflict', conflictHint: 'routines.toast.conflictHint', refresh: () => this.refresh() });
  }

  /** Forgets everything read, for tests and a fresh start. */
  reset(): void {
    this.epoch += 1;
    this.list.sync(null, null);
    this.runs.sync(null, null);
    this.work.reset();
    this.running = new Map();
    this.revision = 0;
    this.wantsNew = false;
    this.openRequest = null;
    this.workspace = null;
    this.reads = 0;
    this.stale = 0;
    this.shown = 0;
  }
}

export const routines = new RoutinesStore();

/** The path routines are read for: the active project, or none. */
export function activeProjectPath(): string | null {
  return project.active?.path ?? null;
}
