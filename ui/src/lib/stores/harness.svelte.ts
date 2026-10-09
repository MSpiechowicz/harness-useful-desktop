import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { prefs, type PrefsStore } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

export type HarnessStatus = 'idle' | 'loading' | 'ready' | 'error';

/** Detection results for every registered harness, and which one is currently in use. */
export class HarnessStore {
  summaries = $state<HarnessSummary[]>([]);
  status = $state<HarnessStatus>('idle');
  error = $state<IpcFailure | null>(null);
  /** True while a re-scan is running. */
  scanning = $state(false);

  /** The harness in use: the daily driver if set, else the first installed one, else the first known. */
  active: HarnessSummary | null = $derived(this.resolveActive());

  constructor(
    private readonly transport: Transport,
    private readonly preferences: Pick<PrefsStore, 'value' | 'update' | 'syncServerManaged'>,
    private readonly notifications: ToastsStore,
  ) {}

  get installed(): HarnessSummary[] {
    return this.summaries.filter((summary) => summary.detection.installed);
  }

  get(id: HarnessId): HarnessSummary | undefined {
    return this.summaries.find((summary) => summary.info.id === id);
  }

  async load(): Promise<void> {
    this.status = 'loading';

    try {
      this.summaries = await this.transport.invoke('harness_list', undefined);
      this.status = 'ready';
      this.error = null;
    } catch (error) {
      this.error = IpcFailure.from(error);
      this.status = 'error';
    }
  }

  /** Re-runs detection for every harness (the user installed one, or fixed their PATH). */
  async rescan(): Promise<void> {
    this.scanning = true;

    try {
      const fresh = await Promise.all(this.summaries.map((summary) => this.transport.invoke('harness_detect', { harness: summary.info.id })));
      this.summaries = fresh;
    } catch (error) {
      this.fail('Could not scan for harnesses', error);
    } finally {
      this.scanning = false;
    }
  }

  /** Opens the native file dialog; returns true when a binary was chosen. */
  async pickBinary(id: HarnessId): Promise<boolean> {
    try {
      const picked = await this.transport.invoke('harness_pick_binary', { harness: id });
      if (!picked) {
        return false;
      }

      this.adopt(picked);
      await this.preferences.syncServerManaged();

      return true;
    } catch (error) {
      this.fail('Could not use that file', error);

      return false;
    }
  }

  async clearBinary(id: HarnessId): Promise<void> {
    try {
      this.adopt(await this.transport.invoke('harness_clear_binary', { harness: id }));
      await this.preferences.syncServerManaged();
    } catch (error) {
      this.fail('Could not reset the binary', error);
    }
  }

  /** Whether the user pinned a custom executable for this harness. */
  hasOverride(id: HarnessId): boolean {
    return (this.preferences.value?.binaryOverrides ?? []).some((override) => override.harness === id);
  }

  /** Makes this harness the default; refuses harnesses that are not installed. */
  async select(id: HarnessId): Promise<void> {
    if (this.get(id)?.detection.installed) {
      await this.preferences.update({ defaultHarness: id });
    }
  }

  private resolveActive(): HarnessSummary | null {
    const preferred = this.summaries.find((summary) => summary.info.id === this.preferences.value?.defaultHarness);

    return preferred ?? this.summaries.find((summary) => summary.detection.installed) ?? this.summaries[0] ?? null;
  }

  private adopt(summary: HarnessSummary): void {
    this.summaries = this.summaries.map((existing) => (existing.info.id === summary.info.id ? summary : existing));
  }

  private fail(title: string, error: unknown): void {
    this.notifications.push({ title, description: IpcFailure.from(error).message, tone: 'danger' });
  }
}

export const harness = new HarnessStore(ipc, prefs, toasts);
