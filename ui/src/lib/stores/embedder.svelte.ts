import { t } from '$lib/i18n/i18n.svelte';
import type { EmbedderPhase } from '$lib/ipc/bindings/EmbedderPhase';
import { IpcFailure } from '$lib/ipc/transport';

import type { ToastsStore } from './toasts.svelte';

/** What every Harness family member's embedder reports about itself. */
export interface EmbedderStateView {
  phase: EmbedderPhase;
  progress: number | null;
  error: string | null;
  modelPresent: boolean | null;
}

/** The commands of one Harness family member's embedder. */
export interface EmbedderCommands<State extends EmbedderStateView> {
  /** Starts following the embedder: its state arrives at once and on every change. */
  watch: (onEvent: (state: State) => void) => Promise<unknown>;
  setVectorSearch: (enabled: boolean) => Promise<{ state: State; vectorSearch: boolean }>;
  removeModel: () => Promise<State>;
}

/** Where the vector search switch is kept: a stored pref of the app, on by default. */
export interface VectorSearchSwitch {
  /** The switch position. */
  readonly on: boolean;
  /** The stored pref without subscribing the caller to it. */
  stored: () => boolean | undefined;
  /** Tells the pref that a change is in flight, so a read in between does not undo it. */
  hold: (pending: boolean) => void;
  store: (on: boolean) => void;
  /** Reads the stored pref again. */
  resync: () => Promise<void>;
}

/** The phases in which the embedder works through something and says how far it is. */
export const WORKING_PHASES = ['starting', 'downloading', 'verifying', 'loading', 'backfilling', 'updating'] as const;

/** A server that predates a command, or is not installed, answers with one of these: the setting is not there to show. */
export function isMissing(error: IpcFailure): boolean {
  return error.code === 'unsupported' || error.code === 'notFound';
}

/**
 * Vector search of a Harness family member, as its Memory-like settings section shows it. The embedder's state follows its watch
 * command. Whether the installed server runs vector search at all is the member's to say (`vectorSupported`).
 */
export abstract class EmbedderStore<State extends EmbedderStateView> {
  /** The embedder right now. Null until the watch said so. */
  state = $state<State | null>(null);
  /** True while the vector search switch is in flight. */
  vectorPending = $state(false);
  /** True while the downloaded model is being deleted. */
  removing = $state(false);
  /** Why the last removal failed. */
  removeFailure = $state<string | null>(null);

  constructor(
    private readonly embedder: EmbedderCommands<State>,
    private readonly toggle: VectorSearchSwitch,
    protected readonly notifications: ToastsStore,
  ) {}

  /** Whether the installed server runs vector search. Unknown counts as no: nothing is shown before it is known. */
  abstract get vectorSupported(): boolean;

  /** True when a refused switch only means the server does not have it: the member remembers that and shows no error. */
  protected refused(_failure: IpcFailure): boolean {
    return false;
  }

  /** The switch position: the stored pref, on by default. */
  get vectorOn(): boolean {
    return this.toggle.on;
  }

  /** How far the work in progress is, from 0 to 1. Null while the embedder works without saying how far. */
  get fraction(): number | null {
    const progress = this.state?.progress;

    return typeof progress === 'number' ? Math.min(1, Math.max(0, progress)) : null;
  }

  /** The embedder is busy with something the user waits for (a download, an index): a bar shows it. */
  get working(): boolean {
    const phase = this.state?.phase;

    return WORKING_PHASES.some((each) => each === phase);
  }

  /** The "Remove downloaded model" action is offered: vector search is off and the server said a model is downloaded. Unknown hides it. */
  get canRemoveModel(): boolean {
    return this.vectorSupported && !this.vectorOn && this.state?.modelPresent === true;
  }

  /**
   * Follows the embedder from now on: its state arrives at once and on every change. Returns the function that stops listening (the backend
   * keeps one listener, so a later start takes over).
   */
  watch(): () => void {
    let listening = true;

    this.embedder
      .watch((state) => {
        if (!listening) {
          return;
        }

        this.state = state;

        // An embedder that is off while the pref says on was switched off by the backend (an opt-out at launch): read the pref again.
        if (state.phase === 'off' && this.toggle.stored() !== false) {
          void this.toggle.resync();
        }
      })
      .catch(() => {
        // The server is missing: there is no embedder to follow, and the switch is hidden by the settings.
      });

    return () => {
      listening = false;
    };
  }

  /** Forgets everything read so far, e.g. for a test. */
  reset(): void {
    this.state = null;
    this.vectorPending = false;
    this.removing = false;
    this.removeFailure = null;
  }

  /** Turns vector search on or off. The switch changes once the server confirms, a refusal leaves it as it was. */
  async setVectorSearch(enabled: boolean): Promise<void> {
    if (this.vectorPending) {
      return;
    }

    this.vectorPending = true;
    this.toggle.hold(true);

    try {
      const switched = await this.embedder.setVectorSearch(enabled);

      this.state = switched.state;
      this.toggle.store(switched.vectorSearch);
    } catch (error) {
      const failure = IpcFailure.from(error);

      if (!this.refused(failure)) {
        this.notifications.push({ title: t('shell.toast.saveFailed'), description: failure.message, tone: 'danger' });
      }
    } finally {
      this.vectorPending = false;
      this.toggle.hold(false);
    }
  }

  /** Deletes the downloaded model and the index. Returns true when they are gone. */
  async removeModel(): Promise<boolean> {
    this.removing = true;
    this.removeFailure = null;

    try {
      this.state = await this.embedder.removeModel();

      return true;
    } catch (error) {
      this.removeFailure = IpcFailure.from(error).message;

      return false;
    } finally {
      this.removing = false;
    }
  }
}
