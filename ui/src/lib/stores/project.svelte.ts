import type { ProjectInfo } from '$lib/ipc/bindings/ProjectInfo';
import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { prefs, type PrefsStore } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

/** The project the user works in, plus the remembered recent ones. Every change goes through the backend. */
export class ProjectStore {
  active = $state<ProjectInfo | null>(null);
  recents = $state<ProjectInfo[]>([]);

  constructor(
    private readonly transport: Transport,
    private readonly preferences: Pick<PrefsStore, 'value'>,
    private readonly notifications: ToastsStore,
  ) {}

  /** Cold start: read the recents and restore the active project saved in the preferences. */
  async load(): Promise<void> {
    await this.refreshRecents();

    const saved = this.preferences.value?.activeProject;
    if (saved) {
      this.active = this.recents.find((project) => project.path === saved) ?? (await this.reopen(saved));
    }
  }

  async refreshRecents(): Promise<void> {
    try {
      this.recents = await this.transport.invoke('project_recent', undefined);
    } catch (error) {
      this.fail('harness.toast.recents', error);
    }
  }

  /** Opens the native folder dialog; returns the project, or null when cancelled or refused. */
  async pick(): Promise<ProjectInfo | null> {
    try {
      const picked = await this.transport.invoke('project_pick', undefined);
      if (picked) {
        this.active = picked;
        await this.refreshRecents();
      }

      return picked;
    } catch (error) {
      this.fail('harness.toast.pickFolder', error);

      return null;
    }
  }

  /** Re-opens a recent project. */
  async open(path: string): Promise<boolean> {
    try {
      this.active = await this.transport.invoke('project_open', { path });
      await this.refreshRecents();

      return true;
    } catch (error) {
      this.fail('harness.toast.openProject', error);

      return false;
    }
  }

  async close(): Promise<void> {
    try {
      await this.transport.invoke('project_close', undefined);
      this.active = null;
    } catch (error) {
      this.fail('harness.toast.closeProject', error);
    }
  }

  async forget(path: string): Promise<void> {
    try {
      this.recents = await this.transport.invoke('project_forget', { path });

      if (this.active?.path === path) {
        this.active = null;
      }
    } catch (error) {
      this.fail('harness.toast.forgetProject', error);
    }
  }

  private async reopen(path: string): Promise<ProjectInfo | null> {
    try {
      return await this.transport.invoke('project_open', { path });
    } catch {
      // The saved folder is gone or no longer allowed; starting without a project is the safe fallback.
      return null;
    }
  }

  private fail(title: MessageKey, error: unknown): void {
    this.notifications.push({ title: t(title), description: IpcFailure.from(error).message, tone: 'danger' });
  }
}

export const project = new ProjectStore(ipc, prefs, toasts);
