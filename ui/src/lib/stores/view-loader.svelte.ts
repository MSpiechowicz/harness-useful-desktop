import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { IpcFailure } from '$lib/ipc/transport';
import { untilStale } from '$lib/utils/flight';

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
  /** The read that is showing the loading state (a first load, a switch, Try again), until it ends. */
  private loading: Promise<void> | null = null;
  private loadingGeneration = 0;

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
   * background: no loading state, a failure keeps what is shown, and data identical to what is shown is not replaced.
   * It never replaces a read that is showing the loading state: it reads once that one has ended, so a first load is
   * not stranded on its loading state by a background read that failed.
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
    this.loading = null;
    this.key = null;
    this.target = null;
    this.data = null;
    this.error = null;
    this.status = 'idle';
  }

  private run(quiet = false): Promise<void> {
    if (!this.target) {
      return Promise.resolve();
    }

    // A background read waits for the read that is loading, instead of replacing it: its failure would leave the loading state on for good.
    if (quiet) {
      return this.loading ? this.readWhenLoaded(this.loading) : this.read(true);
    }

    this.loading = this.read(false);

    return this.loading;
  }

  /** Reads once the loading read has ended, or has not answered for too long (then it is given up on). */
  private async readWhenLoaded(loading: Promise<void>): Promise<void> {
    await untilStale(loading);

    // A newer load took over while this one waited: its answer is the one to show, and a read now would discard it.
    if (this.loading !== null && this.loading !== loading) {
      return;
    }

    this.loading = null;

    return this.read(true);
  }

  private async read(quiet: boolean): Promise<void> {
    const target = this.target;
    if (!target) {
      return;
    }

    this.generation += 1;
    const mine = this.generation;
    if (!quiet) {
      this.status = 'loading';
      this.loadingGeneration = mine;
    }

    try {
      const data = await this.fetcher(target.harness, target.project);

      if (mine === this.generation) {
        // A background read that found the same thing keeps the data on screen, so nothing re-renders and no scroll or selection moves.
        if (!quiet || JSON.stringify(data) !== JSON.stringify(this.data)) {
          this.data = data;
        }

        this.error = null;
        this.status = 'ready';
      }
    } catch (error) {
      // A background read that fails keeps what is shown, but with nothing shown the failure is the page.
      if (mine === this.generation && (!quiet || this.data === null)) {
        this.data = null;
        this.error = IpcFailure.from(error);
        this.status = 'error';
      }
    } finally {
      if (!quiet && this.loadingGeneration === mine) {
        this.loading = null;
      }
    }
  }
}
