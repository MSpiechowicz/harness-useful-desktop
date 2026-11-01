import { untrack } from 'svelte';

import { t } from '$lib/i18n/i18n.svelte';
import type { MemoryEmbedderState } from '$lib/ipc/bindings/MemoryEmbedderState';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { EmbedderStore, isMissing, type VectorSearchSwitch } from './embedder.svelte';
import { memoryRemote, type MemoryRemoteStore } from './memoryRemote.svelte';
import { prefs, type PrefsStore } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

/** The stored `vectorSearch` pref of the app is the switch of Harness Memory's vector search. */
function vectorSearchPref(appPrefs: PrefsStore): VectorSearchSwitch {
  return {
    get on() {
      return appPrefs.value?.vectorSearch ?? true;
    },
    stored: () => untrack(() => appPrefs.value?.vectorSearch),
    hold: (pending) => {
      appPrefs.vectorSearchPending = pending;
    },
    store: (on) => {
      if (appPrefs.value) {
        appPrefs.value = { ...appPrefs.value, vectorSearch: on };
      }
    },
    resync: () => appPrefs.syncServerManaged(),
  };
}

/**
 * Vector search and the English notes setting of Harness Memory, as the Memory section of the settings shows them.
 * The embedder's state follows `memory_embedder_watch`, the settings are read when the section opens. Whether the installed server has
 * either comes from its `features` line (`embedderSupported`, `settingsSupported`, read with the remote memory settings). Only while those
 * are not loaded yet does a rejected call stand in for the answer.
 */
export class MemorySearchStore extends EmbedderStore<MemoryEmbedderState> {
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
    notifications: ToastsStore,
    appPrefs: PrefsStore,
    private readonly remote: MemoryRemoteStore,
  ) {
    super(
      {
        watch: (onEvent) => transport.invoke('memory_embedder_watch', { onEvent }),
        setVectorSearch: (enabled) => transport.invoke('memory_set_vector_search', { enabled }),
        removeModel: () => transport.invoke('memory_embedder_remove_model', undefined),
      },
      vectorSearchPref(appPrefs),
      notifications,
    );
  }

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

  /** A refusal only counts while the server's features are not known: once they are, the switch follows them. */
  protected override refused(failure: IpcFailure): boolean {
    if (isMissing(failure) && this.remote.value === null) {
      this.vectorRefused = true;

      return true;
    }

    return false;
  }

  /** Forgets everything read so far, e.g. for a test. */
  override reset(): void {
    super.reset();
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
      if (isMissing(IpcFailure.from(error))) {
        this.settingsListed = false;
      }
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
