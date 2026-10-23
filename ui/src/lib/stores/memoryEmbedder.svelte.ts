import { untrack } from 'svelte';

import { t } from '$lib/i18n/i18n.svelte';
import type { MemoryEmbedderState } from '$lib/ipc/bindings/MemoryEmbedderState';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { memoryRemote, type MemoryRemoteStore } from './memoryRemote.svelte';
import { prefs, type PrefsStore } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

/** The phases in which the embedder works through something and says how far it is. */
const WORKING = ['starting', 'downloading', 'verifying', 'loading', 'backfilling', 'updating'] as const;

/** A server that predates a command, or is not installed, answers with one of these: the setting is not there to show. */
function missing(error: IpcFailure): boolean {
  return error.code === 'unsupported' || error.code === 'notFound';
}

/**
 * Vector search and the English notes setting of Harness Memory, as the Memory section of the settings shows them.
 * The embedder's state follows `memory_embedder_watch`, the settings are read when the section opens. Whether the installed server has
 * either comes from its `features` line (`embedderSupported`, `settingsSupported`, read with the remote memory settings). Only while those
 * are not loaded yet does a rejected call stand in for the answer.
 */
export class MemorySearchStore {
  /** The embedder right now. Null until the watch said so. */
  state = $state<MemoryEmbedderState | null>(null);
  /** True while `memory_set_vector_search` is in flight. */
  vectorPending = $state(false);
  /** True while the downloaded model is being deleted. */
  removing = $state(false);
  /** Why the last removal failed. */
  removeFailure = $state<string | null>(null);

  /** Whether agents must write notes in English. Null until the server said so. */
  englishNotes = $state<boolean | null>(null);
  englishPending = $state(false);

  /**
   * Whether the server has `settings`, as a call answered or refused: null until it did. Used only while the server's features are not known.
   * The server lists `embedder` and `settings` in the same release, so this also stands for vector search until a refusal says otherwise.
   */
  private settingsListed = $state<boolean | null>(null);
  private vectorRefused = $state(false);

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
    private readonly appPrefs: PrefsStore,
    private readonly remote: MemoryRemoteStore,
  ) {}

  /** Whether the installed Harness Memory runs vector search. Unknown counts as no: nothing is shown before it is known. */
  get vectorSupported(): boolean {
    const listed = this.remote.value?.embedderSupported;

    return listed ?? (this.settingsListed === true && !this.vectorRefused);
  }

  /** Whether the installed Harness Memory has `settings get|set`. Unknown counts as no. */
  get settingsSupported(): boolean {
    const listed = this.remote.value?.settingsSupported;

    return listed ?? (this.settingsListed === true);
  }

  /** The switch position: the stored pref, on by default. */
  get vectorOn(): boolean {
    return this.appPrefs.value?.vectorSearch ?? true;
  }

  /** How far the work in progress is, from 0 to 1. Null while the embedder works without saying how far. */
  get fraction(): number | null {
    const progress = this.state?.progress;

    return typeof progress === 'number' ? Math.min(1, Math.max(0, progress)) : null;
  }

  /** The embedder is busy with something the user waits for (a download, an index): a bar shows it. */
  get working(): boolean {
    const phase = this.state?.phase;

    return WORKING.some((each) => each === phase);
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

    this.transport
      .invoke('memory_embedder_watch', {
        onEvent: (state) => {
          if (!listening) {
            return;
          }

          this.state = state;

          // An embedder that is off while the pref says on was switched off by the backend (an opt-out at launch): read the pref again.
          if (state.phase === 'off' && untrack(() => this.appPrefs.value?.vectorSearch) !== false) {
            void this.appPrefs.syncServerManaged();
          }
        },
      })
      .catch(() => {
        // Harness Memory missing: there is no embedder to follow, and the switch is hidden by the settings.
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
    this.englishNotes = null;
    this.englishPending = false;
    this.settingsListed = null;
    this.vectorRefused = false;
  }

  /** Reads the English notes setting. A server without it, or none installed, hides the switch. */
  async loadSettings(): Promise<void> {
    // A server known to lack `settings` is not asked. Untracked: the effect that reads the settings must not run again with every read of the remote settings.
    if (untrack(() => this.remote.value?.settingsSupported) === false) {
      return;
    }

    try {
      const settings = await this.transport.invoke('memory_settings_get', undefined);

      if (!this.englishPending) {
        this.englishNotes = settings.englishNotes;
      }

      this.settingsListed = true;
    } catch (error) {
      if (missing(IpcFailure.from(error))) {
        this.settingsListed = false;
      }
    }
  }

  /** Turns vector search on or off. The switch changes once the server confirms, a refusal leaves it as it was. */
  async setVectorSearch(enabled: boolean): Promise<void> {
    if (this.vectorPending) {
      return;
    }

    this.vectorPending = true;
    this.appPrefs.vectorSearchPending = true;

    try {
      const switched = await this.transport.invoke('memory_set_vector_search', { enabled });

      this.state = switched.state;

      if (this.appPrefs.value) {
        this.appPrefs.value = { ...this.appPrefs.value, vectorSearch: switched.vectorSearch };
      }
    } catch (error) {
      const failure = IpcFailure.from(error);

      // A refusal only counts while the server's features are not known: once they are, the switch follows them.
      if (missing(failure) && this.remote.value === null) {
        this.vectorRefused = true;
      } else {
        this.notifications.push({ title: t('shell.toast.saveFailed'), description: failure.message, tone: 'danger' });
      }
    } finally {
      this.vectorPending = false;
      this.appPrefs.vectorSearchPending = false;
    }
  }

  /** Deletes the downloaded model and the index. Returns true when they are gone. */
  async removeModel(): Promise<boolean> {
    this.removing = true;
    this.removeFailure = null;

    try {
      this.state = await this.transport.invoke('memory_embedder_remove_model', undefined);

      return true;
    } catch (error) {
      this.removeFailure = IpcFailure.from(error).message;

      return false;
    } finally {
      this.removing = false;
    }
  }

  /** Turns English notes on or off for everyone who uses the store. The switch changes once the server confirms. */
  async setEnglishNotes(englishNotes: boolean): Promise<void> {
    if (this.englishPending) {
      return;
    }

    this.englishPending = true;

    try {
      this.englishNotes = (await this.transport.invoke('memory_settings_set', { englishNotes })).englishNotes;
    } catch (error) {
      this.notifications.push({ title: t('shell.toast.saveFailed'), description: IpcFailure.from(error).message, tone: 'danger' });
    } finally {
      this.englishPending = false;
    }
  }
}

export const memorySearch = new MemorySearchStore(ipc, toasts, prefs, memoryRemote);
