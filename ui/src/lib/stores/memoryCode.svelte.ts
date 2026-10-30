import { i18n, t, tn } from '$lib/i18n/i18n.svelte';
import type { CodeBuildEvent } from '$lib/ipc/bindings/CodeBuildEvent';
import type { CodeIndexState } from '$lib/ipc/bindings/CodeIndexState';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { toasts, type ToastInput, type ToastsStore } from './toasts.svelte';

/** A new build of a repository that began within this long after its last toast is not announced again (an error is always announced). */
const REPEAT_QUIET_MS = 10 * 60 * 1000;
/** A finished build's toast stays this long. */
const READY_TOAST_MS = 4000;
/** What is known of a build that ended, and of a repository's last toast, is forgotten after this long. */
const FORGET_AFTER_MS = 30 * 60 * 1000;

/** What the controller knows about a build it was told of. A build has at most one toast at a time. */
interface BuildRecord {
  /**
   * `open`: its toast is shown. `closed`: it finished or was dropped. `paused`: it went quiet (`stale`) and its toast was taken away, a
   * later event of it shows one toast again. `suppressed`: it began too soon after a toast of the same repository, so none of its events
   * is shown (but an error).
   */
  status: 'open' | 'closed' | 'paused' | 'suppressed';
  /** What the last event said, so the same event again changes nothing. */
  mark: string;
  /** When the build was last told of. */
  at: number;
}

export interface BuildToastState {
  readonly builds: Readonly<Record<number, BuildRecord>>;
  /** When a toast about a repository root was last shown or changed. */
  readonly shownAt: Readonly<Record<string, number>>;
}

export const EMPTY_BUILD_TOASTS: BuildToastState = { builds: {}, shownAt: {} };

export type BuildToastDecision =
  | { action: 'push'; toast: ToastInput; state: BuildToastState }
  | { action: 'update'; toast: Partial<ToastInput>; state: BuildToastState }
  | { action: 'dismiss'; state: BuildToastState }
  | { action: 'none'; state: BuildToastState };

function markOf(event: CodeBuildEvent): string {
  return `${event.phase}:${event.filesDone}/${event.filesTotal}:${event.error ?? ''}`;
}

function number(count: number): string {
  return new Intl.NumberFormat(i18n.locale).format(count);
}

/** "12 of 240 files". Nothing while the files are not counted yet: there is no number to show. */
function progressText(event: CodeBuildEvent): string | undefined {
  return event.filesTotal > 0 ? tn('shell.toast.codeGraph.progress', event.filesTotal, { done: number(event.filesDone) }) : undefined;
}

/** "25 files indexed in my-repo". Nothing while the files are not counted: there is no number to show. */
function readyText(event: CodeBuildEvent): string | undefined {
  return event.filesTotal > 0 ? tn('shell.toast.codeGraph.indexed', event.filesTotal, { repo: event.repoName }) : undefined;
}

/** The toast a build is shown as in its phase, or null for a phase that has none (the build is dropped). */
function shownAs(event: CodeBuildEvent): ToastInput | null {
  switch (event.phase) {
    case 'building':
      return { title: t('shell.toast.codeGraph.building', { repo: event.repoName }), description: progressText(event), duration: 0 };
    case 'ready':
      return { title: t('shell.toast.codeGraph.readyPlain'), description: readyText(event), tone: 'success', duration: READY_TOAST_MS };
    case 'error':
      return { title: t('shell.toast.codeGraph.error'), description: event.error ?? undefined, tone: 'danger', duration: 0 };
    default:
      return null;
  }
}

/** `touch`: the toast of the repository was shown or changed now, which starts its quiet time. */
function remember(state: BuildToastState, event: CodeBuildEvent, status: BuildRecord['status'], now: number, touch: boolean): BuildToastState {
  return {
    builds: { ...state.builds, [event.buildId]: { status, mark: markOf(event), at: now } },
    shownAt: touch ? { ...state.shownAt, [event.root]: now } : state.shownAt,
  };
}

/** Forgets the builds that ended and the repositories whose last toast is long past. A build with a toast is kept as long as it goes on. */
function prune(state: BuildToastState, now: number): BuildToastState {
  const old = (at: number): boolean => now - at > FORGET_AFTER_MS;
  const builds = Object.entries(state.builds).filter(([, record]) => record.status === 'open' || !old(record.at));
  const shownAt = Object.entries(state.shownAt).filter(([, at]) => !old(at));

  if (builds.length === Object.keys(state.builds).length && shownAt.length === Object.keys(state.shownAt).length) {
    return state;
  }

  return { builds: Object.fromEntries(builds), shownAt: Object.fromEntries(shownAt) };
}

/** The toast of a build that has none now. `quiet`: a build that began too soon after the last toast of its repository gets none (never an error). */
function show(event: CodeBuildEvent, state: BuildToastState, now: number, quiet: boolean): BuildToastDecision {
  const toast = shownAs(event);

  // A build that stopped before it was ever shown has nothing to take back. A cancelled one is over: it never shows again.
  if (toast === null) {
    const ended = event.phase === 'cancelled' ? remember(state, event, 'closed', now, false) : state;

    return { action: 'none', state: ended };
  }

  const last = state.shownAt[event.root];

  if (quiet && event.phase !== 'error' && last !== undefined && now - last < REPEAT_QUIET_MS) {
    return { action: 'none', state: remember(state, event, 'suppressed', now, false) };
  }

  // A build seen only when it was done (it finished between two scans of the backend) gets one short toast.
  const open = event.phase === 'building';

  // No `project`: the toast follows the build, whichever project the user is in.
  return { action: 'push', toast, state: remember(state, event, open ? 'open' : 'closed', now, true) };
}

/** The next event of a build whose toast is shown. */
function goOn(event: CodeBuildEvent, state: BuildToastState, now: number): BuildToastDecision {
  const toast = shownAs(event);

  if (toast === null) {
    // `stale` is a pause (the build may write again), anything else ends the build.
    return { action: 'dismiss', state: remember(state, event, event.phase === 'stale' ? 'paused' : 'closed', now, false) };
  }

  const finished = event.phase !== 'building';

  // The same toast changes in place: what a phase does not say is cleared, so no text of an earlier phase stays.
  return { action: 'update', toast: { description: undefined, ...toast }, state: remember(state, event, finished ? 'closed' : 'open', now, true) };
}

/**
 * What to do with a code graph build event: show a toast for a build not seen before, change it as the build goes on, or take it away.
 * Pure: the caller keeps the returned `state` and carries out the action. A repeated event, and any event of a build that is closed or
 * suppressed, does nothing, except that a build which went quiet shows one toast again when it writes again, and an error is always shown.
 */
export function decide(event: CodeBuildEvent, previous: BuildToastState, now: number): BuildToastDecision {
  const state = prune(previous, now);
  const known = state.builds[event.buildId];

  if (known === undefined) {
    return show(event, state, now, true);
  }

  if (known.mark === markOf(event)) {
    return { action: 'none', state };
  }

  switch (known.status) {
    case 'open':
      return goOn(event, state, now);
    case 'paused':
      return show(event, state, now, false);
    case 'suppressed':
      return event.phase === 'error' ? show(event, state, now, false) : { action: 'none', state };
    case 'closed':
      return { action: 'none', state };
  }
}

/** A server that predates the code index, or is not installed, answers with one of these: the switch is not there to show. */
function missing(error: IpcFailure): boolean {
  return error.code === 'unsupported' || error.code === 'notFound';
}

/**
 * The code index of Harness Memory: its switch in the settings, and the toast of each code graph build. The builds arrive from
 * `memory_code_builds_listen` (the backend sends the running ones again when the page listens anew), and a build is one toast that
 * changes in place.
 */
export class MemoryCodeStore {
  /** The switch as the server last reported it. Null until it did, and for a server without a code index: nothing is shown then. */
  index = $state<CodeIndexState | null>(null);
  /** True while `memory_code_index_set` is in flight. */
  pending = $state(false);
  /** Why the last change was refused, shown in the row until the next try. */
  failure = $state<string | null>(null);

  private builds: BuildToastState = EMPTY_BUILD_TOASTS;
  /** Counts the changes of the switch, so a read that began before the latest one does not overwrite it. */
  private changes = 0;
  /** The toast of each build that has one. */
  private shown = new Map<number, number>();
  /** The projects `prepare` was asked for in this run: each is asked once, until the switch is turned on again. */
  private prepared = new Set<string>();

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
    private readonly clock: () => number = Date.now,
  ) {}

  /** Whether the environment decides the switch (`source` is `env`): it cannot be changed here. */
  get locked(): boolean {
    return this.index?.source === 'env';
  }

  /** Reads the switch. A server without a code index hides it. */
  async load(): Promise<void> {
    if (this.pending) {
      return;
    }

    const started = this.changes;

    try {
      const read = await this.transport.invoke('memory_code_index_get', undefined);

      this.index = started === this.changes ? read : this.index;
    } catch {
      // Not installed, too old, or unreadable: there is no switch to show.
      this.index = started === this.changes ? null : this.index;
    }
  }

  /** Turns the code index on or off. The switch moves at once and goes back if the server refuses. */
  async setEnabled(on: boolean): Promise<void> {
    const before = this.index;

    if (before === null || this.pending || this.locked) {
      return;
    }

    this.changes += 1;
    this.pending = true;
    this.failure = null;
    this.index = { ...before, enabled: on };

    // A build the switch refused while it was off is asked for again once it is on.
    if (on && !before.enabled) {
      this.prepared.clear();
    }

    try {
      this.index = await this.transport.invoke('memory_code_index_set', { on });
    } catch (error) {
      const failure = IpcFailure.from(error);

      this.index = missing(failure) ? null : before;
      this.failure = missing(failure) ? null : failure.message;
    } finally {
      this.pending = false;
    }
  }

  /** Follows the builds from now on. Returns the function that stops listening (the backend keeps one listener, so a later start takes over). */
  startBuilds(): () => void {
    let listening = true;

    this.transport
      .invoke('memory_code_builds_listen', {
        onEvent: (event) => {
          if (listening) {
            this.handle(event);
          }
        },
      })
      .catch(() => {
        // Harness Memory missing or without a code index: there are no builds to follow.
      });

    return () => {
      listening = false;
    };
  }

  /**
   * Asks the backend to index a project that has no code index yet, in the background (the build shows as a toast). Each project is asked
   * once, and nothing happens for none. A refusal is ignored: the server may lack a code index, and the switch says so.
   */
  prepare(path: string | null): void {
    if (path === null || this.prepared.has(path)) {
      return;
    }

    this.prepared.add(path);

    this.transport.invoke('memory_code_prepare', { project: path }).catch(() => {
      // Not installed, too old, or the build did not start: the project is indexed on the next run.
    });
  }

  /** Shows, changes or removes the toast of a build as `decide` says. */
  handle(event: CodeBuildEvent): void {
    const decision = decide(event, this.builds, this.clock());
    const id = this.shown.get(event.buildId);

    this.builds = decision.state;

    // The toast ids of builds that are forgotten are not needed any more.
    for (const known of this.shown.keys()) {
      if (known !== event.buildId && this.builds.builds[known] === undefined) {
        this.shown.delete(known);
      }
    }

    switch (decision.action) {
      case 'push':
        this.shown.set(event.buildId, this.notifications.push(decision.toast));
        break;
      case 'update':
        if (id !== undefined) {
          this.notifications.update(id, decision.toast);
        }

        break;
      case 'dismiss':
        if (id !== undefined) {
          this.notifications.dismiss(id);
        }

        break;
      case 'none':
        break;
    }
  }

  /** Forgets everything read so far, e.g. for a test. */
  reset(): void {
    this.changes += 1;
    this.index = null;
    this.pending = false;
    this.failure = null;
    this.builds = EMPTY_BUILD_TOASTS;
    this.shown.clear();
    this.prepared.clear();
  }
}

export const memoryCode = new MemoryCodeStore(ipc, toasts);
