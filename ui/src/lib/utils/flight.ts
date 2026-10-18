import { FLIGHT_STALE_MS } from '$lib/stores/live';

/**
 * Remembers the read that is still running, so a poll tick that finds one skips instead of starting a second:
 * a slow disk or process is never asked twice at once. Tasks given to `start` should not reject (they report
 * their own failures), so the remembered promise always settles quietly.
 *
 * A read that has not answered after `FLIGHT_STALE_MS` is abandoned: it no longer counts as running, so the next
 * one starts. The store must ignore a late answer of an abandoned read once a newer one has been applied
 * (every store here already keys its answers by a generation).
 */
export class Flight {
  private current: Promise<void> | null = null;
  private abandonTimer: ReturnType<typeof setTimeout> | null = null;

  /** The read in flight, or null when none is running. */
  get running(): Promise<void> | null {
    return this.current;
  }

  /** Runs `task` and keeps it as the read in flight until it settles. A newer task replaces an older one. */
  start(task: () => Promise<void>): Promise<void> {
    this.forget();

    const mine: Promise<void> = task().finally(() => {
      if (this.current === mine) {
        this.forget();
      }
    });

    this.current = mine;
    this.abandonTimer = setTimeout(() => {
      if (this.current === mine) {
        this.forget();
      }
    }, FLIGHT_STALE_MS);

    return mine;
  }

  /** Forgets the read in flight (the store was reset), so the next one starts fresh instead of joining it. */
  clear(): void {
    this.forget();
  }

  /** Like `start`, but returns the read already in flight when there is one instead of starting another. */
  startUnlessRunning(task: () => Promise<void>): Promise<void> {
    return this.current ?? this.start(task);
  }

  private forget(): void {
    this.current = null;

    if (this.abandonTimer !== null) {
      clearTimeout(this.abandonTimer);
      this.abandonTimer = null;
    }
  }
}

/** Waits for `promise`, but no longer than `FLIGHT_STALE_MS`: whoever waits on a read that never answers goes on. */
export function untilStale(promise: Promise<unknown>): Promise<void> {
  let timer: ReturnType<typeof setTimeout>;
  const stale = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, FLIGHT_STALE_MS);
  });

  return Promise.race([promise.then(() => undefined, () => undefined), stale]).finally(() => clearTimeout(timer));
}
