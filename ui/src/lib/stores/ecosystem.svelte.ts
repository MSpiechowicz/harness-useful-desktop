import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import type { SlotStatus } from './dashboard.svelte';

/** A status read this recent is shown as is when the view is opened again. */
const FRESH_MS = 30_000;

/** Which companion tools of the Harness Useful ecosystem are installed on this machine. */
export class EcosystemStore {
  status = $state.raw<EcosystemStatus | null>(null);
  state = $state<SlotStatus>('idle');
  error = $state<string | null>(null);

  private loadedAt = 0;
  private token = 0;

  constructor(
    private readonly transport: Transport,
    private readonly now: () => number = Date.now,
  ) {}

  /** Forgets the last reading, as at startup (and between tests). */
  reset(): void {
    this.token += 1;
    this.status = null;
    this.error = null;
    this.state = 'idle';
    this.loadedAt = 0;
  }

  /** Reads the detection result unless a fresh one is held; `force` always reads. */
  async load(force = false): Promise<void> {
    if (!force && this.status && this.now() - this.loadedAt < FRESH_MS) {
      return;
    }

    this.token += 1;
    const mine = this.token;
    this.state = 'loading';

    try {
      const status = await this.transport.invoke('ecosystem_status', undefined);

      if (mine === this.token) {
        this.status = status;
        this.error = null;
        this.state = 'ready';
        this.loadedAt = this.now();
      }
    } catch (error) {
      if (mine === this.token) {
        this.status = null;
        this.error = IpcFailure.from(error).message;
        this.state = 'error';
      }
    }
  }
}

export const ecosystem = new EcosystemStore(ipc);
