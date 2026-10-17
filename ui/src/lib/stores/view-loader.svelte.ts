import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { IpcFailure } from '$lib/ipc/transport';

export type ViewStatus = 'idle' | 'loading' | 'ready' | 'error';

type Fetcher<T, K extends string> = (key: K, project: string | null) => Promise<T>;

/**
 * Loads one view (config, extensions) for a (key, project) pair. The key is the harness unless a view says otherwise
 * (the memory scripts have no harness, and the detail of one script is keyed by its id).
 * `sync` is idempotent: it only fetches when the pair differs from the one already loaded,
 * and a slow response for a stale pair never overwrites a newer one.
 */
export class ViewLoader<T, K extends string = HarnessId> {
  data = $state.raw<T | null>(null);
  status = $state<ViewStatus>('idle');
  error = $state<IpcFailure | null>(null);

  private key: string | null = null;
  private target: { harness: K; project: string | null } | null = null;
  private generation = 0;

  constructor(private readonly fetcher: Fetcher<T, K>) {}

  /** Makes the loaded view match the given pair; `null` harness clears it. */
  sync(harness: K | null, project: string | null): void {
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

  /**
   * Re-fetches the current pair; resolves once the new data (or the error) is in place. A `quiet` refresh runs in the
   * background: no loading state, and a failure keeps what is shown.
   */
  refresh({ quiet = false } = {}): Promise<void> {
    return this.target ? this.run(quiet) : Promise.resolve();
  }

  /** Like `sync`, but a pair already loaded is quietly re-read, so a view shows what changed while it was closed. */
  revalidate(harness: K | null, project: string | null): void {
    const loaded = harness !== null && this.key === `${harness}\u0000${project ?? ''}`;

    if (loaded) {
      void this.refresh({ quiet: true });
    } else {
      this.sync(harness, project);
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

  private async run(quiet = false): Promise<void> {
    const target = this.target;
    if (!target) {
      return;
    }

    this.generation += 1;
    const mine = this.generation;
    if (!quiet) {
      this.status = 'loading';
    }

    try {
      const data = await this.fetcher(target.harness, target.project);

      if (mine === this.generation) {
        this.data = data;
        this.error = null;
        this.status = 'ready';
      }
    } catch (error) {
      if (mine === this.generation && !quiet) {
        this.data = null;
        this.error = IpcFailure.from(error);
        this.status = 'error';
      }
    }
  }
}
