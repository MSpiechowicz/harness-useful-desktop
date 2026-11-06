import { t } from '$lib/i18n/i18n.svelte';
import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import type { ManualRemoval } from '$lib/ipc/bindings/ManualRemoval';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { toasts, type ToastsStore } from './toasts.svelte';

export type PrefsStatus = 'idle' | 'loading' | 'ready' | 'error';

export class PrefsStore {
  value = $state<AppPrefs | null>(null);
  status = $state<PrefsStatus>('idle');
  /** True while `memory_set_enabled` is in flight. */
  memoryPending = $state(false);
  /** True while `memory_set_vector_search` is in flight (set by the memory search store). */
  vectorSearchPending = $state(false);
  /** True while `knowledge_set_enabled` is in flight. */
  knowledgePending = $state(false);
  /** True while `knowledge_set_vector_search` is in flight (set by the knowledge search store). */
  knowledgeVectorSearchPending = $state(false);
  /** True while `code_set_enabled` is in flight. */
  codePending = $state(false);
  /** Entries the last "off" switch left in files with comments: the user removes them by hand, so they stay listed until dismissed. */
  memoryManualRemovals = $state<ManualRemoval[]>([]);
  /** The same for Project knowledge. */
  knowledgeManualRemovals = $state<ManualRemoval[]>([]);
  /** The same for the code graph. */
  codeManualRemovals = $state<ManualRemoval[]>([]);
  error = $state<IpcFailure | null>(null);

  /** Incremented per update so only the newest request may overwrite or roll back state. */
  private revision = 0;

  /** The load in flight, if any. */
  private loading: Promise<void> | null = null;

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
  ) {}

  /** Concurrent calls share one request, so two reads can never resolve out of order. */
  load(): Promise<void> {
    this.loading ??= this.read().finally(() => {
      this.loading = null;
    });

    return this.loading;
  }

  private async read(): Promise<void> {
    this.status = 'loading';

    try {
      this.value = await this.transport.invoke('prefs_get', undefined);
      this.status = 'ready';
      this.error = null;
    } catch (error) {
      this.error = IpcFailure.from(error);
      this.status = 'error';
    }
  }

  /**
   * Re-reads the fields the backend owns (active project, recent projects, binary overrides, project memory, knowledge and the code graph, vector search)
   * without disturbing the UI-managed ones or an in-flight update.
   */
  async syncServerManaged(): Promise<void> {
    try {
      const fresh = await this.transport.invoke('prefs_get', undefined);

      if (this.value) {
        this.value = {
          ...this.value,
          activeProject: fresh.activeProject,
          recentProjects: fresh.recentProjects,
          binaryOverrides: fresh.binaryOverrides,
          // A switch in flight decides the value once it returns.
          memoryEnabled: this.memoryPending ? this.value.memoryEnabled : fresh.memoryEnabled,
          // The backend turns it off by itself when the embedder is opted out at launch.
          vectorSearch: this.vectorSearchPending ? this.value.vectorSearch : fresh.vectorSearch,
          knowledgeEnabled: this.knowledgePending ? this.value.knowledgeEnabled : fresh.knowledgeEnabled,
          knowledgeVectorSearch: this.knowledgeVectorSearchPending ? this.value.knowledgeVectorSearch : fresh.knowledgeVectorSearch,
          codeEnabled: this.codePending ? this.value.codeEnabled : fresh.codeEnabled,
        };
      }
    } catch {
      // Cosmetic data (e.g. whether "Reset binary" shows); the next full load repairs it.
    }
  }

  /**
   * The "Project memory" switch. `memoryEnabled` is server-managed, so it goes through its own command, never
   * `prefs_set`; only that field is taken from the answer so an in-flight optimistic update is not disturbed.
   * The value changes once the backend confirms; a failure leaves it as it was and raises a toast.
   */
  async setMemoryEnabled(enabled: boolean): Promise<void> {
    if (this.memoryPending || !this.value) {
      return;
    }

    this.memoryPending = true;

    try {
      const saved = await this.transport.invoke('memory_set_enabled', { enabled });

      this.memoryManualRemovals = saved.manualRemovals;
      this.keepServerSwitch({ memoryEnabled: saved.memoryEnabled });
    } catch (error) {
      this.switchFailed(error);
    } finally {
      this.memoryPending = false;
    }
  }

  /** The "Project knowledge" switch, which works like the memory one through `knowledge_set_enabled`. */
  async setKnowledgeEnabled(enabled: boolean): Promise<void> {
    if (this.knowledgePending || !this.value) {
      return;
    }

    this.knowledgePending = true;

    try {
      const saved = await this.transport.invoke('knowledge_set_enabled', { enabled });

      this.knowledgeManualRemovals = saved.manualRemovals;
      this.keepServerSwitch({ knowledgeEnabled: saved.knowledgeEnabled });
    } catch (error) {
      this.switchFailed(error);
    } finally {
      this.knowledgePending = false;
    }
  }

  /** The "Code graph" setting, which works like the memory one through `code_set_enabled`. True when the backend stored it. */
  async setCodeEnabled(enabled: boolean): Promise<boolean> {
    if (this.codePending || !this.value) {
      return false;
    }

    this.codePending = true;

    try {
      const saved = await this.transport.invoke('code_set_enabled', { enabled });

      this.codeManualRemovals = saved.manualRemovals;
      this.keepServerSwitch({ codeEnabled: saved.codeEnabled });

      return true;
    } catch (error) {
      this.switchFailed(error);

      return false;
    } finally {
      this.codePending = false;
    }
  }

  /** The one-time memory notice was handled: only this field is taken from the stored preferences, so an update in flight is not disturbed. */
  keepMemoryOptInNoticeDone(done: boolean): void {
    this.keepServerSwitch({ memoryOptInNoticeDone: done });
  }

  /** Takes only the switch's own field from its answer. */
  private keepServerSwitch(fields: Partial<AppPrefs>): void {
    if (this.value) {
      this.value = { ...this.value, ...fields };
    }
  }

  private switchFailed(error: unknown): void {
    this.notifications.push({
      title: t('shell.toast.saveFailed'),
      description: IpcFailure.from(error).message,
      tone: 'danger',
    });
  }

  /** The user dealt with the entries listed by the last switch (or does not want the note any more). */
  dismissMemoryManualRemovals(): void {
    this.memoryManualRemovals = [];
  }

  dismissKnowledgeManualRemovals(): void {
    this.knowledgeManualRemovals = [];
  }

  dismissCodeManualRemovals(): void {
    this.codeManualRemovals = [];
  }

  /** Applies the patch immediately, persists it, and rolls back with a toast if the backend rejects it. */
  async update(patch: Partial<AppPrefs>): Promise<void> {
    const before = this.value;
    if (!before) {
      return;
    }

    const optimistic = { ...before, ...patch };
    this.revision += 1;
    const mine = this.revision;
    this.value = optimistic;

    try {
      const saved = await this.transport.invoke('prefs_set', { prefs: optimistic });

      if (mine === this.revision) {
        this.value = saved;
      }
    } catch (error) {
      if (mine === this.revision) {
        this.value = before;
      }

      this.notifications.push({
        title: t('shell.toast.saveFailed'),
        description: IpcFailure.from(error).message,
        tone: 'danger',
      });
    }
  }
}

export const prefs = new PrefsStore(ipc, toasts);
