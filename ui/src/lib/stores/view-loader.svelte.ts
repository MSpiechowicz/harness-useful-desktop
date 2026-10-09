import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { IpcFailure } from '$lib/ipc/transport';

export type ViewStatus = 'idle' | 'loading' | 'ready' | 'error';

type Fetcher<T> = (harness: HarnessId, project: string | null) => Promise<T>;

/**
 * Loads one harness view (config, extensions) for a (harness, project) pair.
 * `sync` is idempotent: it only fetches when the pair differs from the one already loaded,
 * and a slow response for a stale pair never overwrites a newer one.
 */
export class ViewLoader<T> {
  data = $state.raw<T | null>(null);
  status = $state<ViewStatus>('idle');
  error = $state<IpcFailure | null>(null);

  private key: string | null = null;
  private target: { harness: HarnessId; project: string | null } | null = null;
  private generation = 0;

  constructor(private readonly fetcher: Fetcher<T>) {}

  /** Makes the loaded view match the given pair; `null` harness clears it. */
  sync(harness: HarnessId | null, project: string | null): void {
    if (harness === null) {
      this.reset();

      return;
    }

    const key = `${harness}\u0000${project ?? ''}`;
    if (key === this.key) {
      return;
    }

    this.key = key;
    this.target = { harness, project };
    void this.run();
  }

  /** Re-fetches the current pair. */
  refresh(): void {
    if (this.target) {
      void this.run();
    }
  }

  private reset(): void {
    this.generation += 1;
    this.key = null;
    this.target = null;
    this.data = null;
    this.error = null;
    this.status = 'idle';
  }

  private async run(): Promise<void> {
    const target = this.target;
    if (!target) {
      return;
    }

    this.generation += 1;
    const mine = this.generation;
    this.status = 'loading';

    try {
      const data = await this.fetcher(target.harness, target.project);

      if (mine === this.generation) {
        this.data = data;
        this.error = null;
        this.status = 'ready';
      }
    } catch (error) {
      if (mine === this.generation) {
        this.data = null;
        this.error = IpcFailure.from(error);
        this.status = 'error';
      }
    }
  }
}
