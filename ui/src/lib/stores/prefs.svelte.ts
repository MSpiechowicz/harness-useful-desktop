import { t } from '$lib/i18n/i18n.svelte';
import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { toasts, type ToastsStore } from './toasts.svelte';

export type PrefsStatus = 'idle' | 'loading' | 'ready' | 'error';

export class PrefsStore {
  value = $state<AppPrefs | null>(null);
  status = $state<PrefsStatus>('idle');
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
   * Re-reads the fields the backend owns (active project, recent projects, binary overrides)
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
        };
      }
    } catch {
      // Cosmetic data (e.g. whether "Reset binary" shows); the next full load repairs it.
    }
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
