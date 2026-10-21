import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import { openCodeGeneration, type OpenCodeGeneration } from '$lib/ipc/harness-ids';
import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
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

  /** The harness in use: the daily driver if set, else the first installed one, else the first known. */
  active: HarnessSummary | null = $derived(this.resolveActive());

  /**
   * The harness a new chat talks to: the daily driver when it can chat, else the first installed harness that can.
   * Views follow `active`, which may be a harness without chat (OpenCode), but a chat never starts on one.
   * Null only when the harness list holds nothing that can chat.
   */
  chatActive: HarnessSummary | null = $derived(this.resolveChat());

  /**
   * Set when harnesses are installed but none of them can chat yet (an OpenCode-only machine): the one to name
   * in "chat is coming soon". Null when a chat is possible, or when nothing is installed at all.
   */
  chatComingSoon: HarnessSummary | null = $derived(this.resolveComingSoon());

  private scan: Promise<void> | null = null;
  private scannedAt = 0;
  /** Counts the summaries adopted from a pick or a clear, and remembers when each harness last got one. */
  private adoptions = 0;
  private adoptedAt = new Map<HarnessId, number>();

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

  /** The generation of the OpenCode on this machine, from its detected version. */
  get openCodeGeneration(): OpenCodeGeneration {
    return openCodeGeneration(this.get('opencode')?.detection.version);
  }

  /** Whether the app can drive a chat with this harness. A harness the list does not know is not asked about, so it counts. */
  canChat(id: HarnessId): boolean {
    return this.get(id)?.capabilities.chat ?? true;
  }

  async load(): Promise<void> {
    this.status = 'loading';

    try {
      this.summaries = await this.transport.invoke('harness_list', undefined);
      this.scannedAt = Date.now();
      this.status = 'ready';
      this.error = null;
    } catch (error) {
      this.error = IpcFailure.from(error);
      this.status = 'error';
    }
  }

  /**
   * Re-runs detection for every harness (the user installed one, or fixed their PATH). It runs in the background: no
   * spinner, no message when it fails, and the list is only replaced when something changed.
   */
  async rescan(): Promise<void> {
    if (this.scan !== null) {
      return this.scan;
    }

    this.scan = this.detect().finally(() => {
      this.scan = null;
      this.scannedAt = Date.now();
    });

    return this.scan;
  }

  /** Like `rescan`, unless detection ran within `gapMs` already: coming back to the window over and over costs one scan. */
  async rescanIfStale(gapMs: number): Promise<void> {
    if (this.status === 'ready' && Date.now() - this.scannedAt >= gapMs) {
      await this.rescan();
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
      this.fail('harness.toast.pickBinary', error);

      return false;
    }
  }

  async clearBinary(id: HarnessId): Promise<void> {
    try {
      this.adopt(await this.transport.invoke('harness_clear_binary', { harness: id }));
      await this.preferences.syncServerManaged();
    } catch (error) {
      this.fail('harness.toast.clearBinary', error);
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

  private resolveChat(): HarnessSummary | null {
    const chatty = this.summaries.filter((summary) => summary.capabilities.chat);
    const preferred = chatty.find((summary) => summary.info.id === this.preferences.value?.defaultHarness);

    return preferred ?? chatty.find((summary) => summary.detection.installed) ?? chatty[0] ?? null;
  }

  private resolveComingSoon(): HarnessSummary | null {
    const installed = this.summaries.filter((summary) => summary.detection.installed);
    if (installed.length === 0 || installed.some((summary) => summary.capabilities.chat)) {
      return null;
    }

    return installed.find((summary) => summary.info.id === this.preferences.value?.defaultHarness) ?? installed[0];
  }

  private async detect(): Promise<void> {
    try {
      const started = this.adoptions;
      const detected = await Promise.all(this.summaries.map((summary) => this.transport.invoke('harness_detect', { harness: summary.info.id })));
      // A binary picked or cleared while the scan ran is newer than what the scan saw: that harness keeps its summary.
      const fresh = detected.map((summary) => (this.adoptedSince(summary.info.id, started) ? (this.get(summary.info.id) ?? summary) : summary));

      if (JSON.stringify(fresh) !== JSON.stringify(this.summaries)) {
        this.summaries = fresh;
      }
    } catch {
      // The next look tries again, and what is shown stays.
    }
  }

  private adoptedSince(id: HarnessId, adoptions: number): boolean {
    return (this.adoptedAt.get(id) ?? 0) > adoptions;
  }

  private adopt(summary: HarnessSummary): void {
    this.adoptions += 1;
    this.adoptedAt.set(summary.info.id, this.adoptions);
    this.summaries = this.summaries.map((existing) => (existing.info.id === summary.info.id ? summary : existing));
  }

  private fail(title: MessageKey, error: unknown): void {
    this.notifications.push({ title: t(title), description: IpcFailure.from(error).message, tone: 'danger' });
  }
}

export const harness = new HarnessStore(ipc, prefs, toasts);
