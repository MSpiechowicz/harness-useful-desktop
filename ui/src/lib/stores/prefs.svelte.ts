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

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
  ) {}

  async load(): Promise<void> {
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
        title: 'Could not save settings',
        description: IpcFailure.from(error).message,
        tone: 'danger',
      });
    }
  }
}

export const prefs = new PrefsStore(ipc, toasts);
