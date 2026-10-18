import { FLIGHT_STALE_MS } from '$lib/stores/live';

import { autoRetry } from './auto-retry';

export interface AutoRefreshOptions {
  /**
   * The view is showing a failure. The steady poll then gives way to the retry schedule (`RETRY_BACKOFF_MS`: a little
   * later each time), so a view has one mechanism, not a poll with a retry on top. Read it inside the `$effect` that
   * starts the poll: when it flips, the effect restarts the poll in the other mode.
   */
  failing?: boolean;
}

/**
 * Calls `run` every `ms` while the window is visible, and again whenever it regains focus, so an open view shows what
 * changed on disk without anyone asking. A tick that finds the previous `run` still going (it returned a promise that
 * has not settled) is skipped, so a slow read is never asked twice at once. A run that has not settled after
 * `FLIGHT_STALE_MS` is given up on: the next tick goes ahead, and until a run settles again the poll counts as failing
 * and follows the retry schedule. Returns the cleanup for an `$effect`.
 */
export function autoRefresh(run: () => unknown, ms: number, { failing = false }: AutoRefreshOptions = {}): () => void {
  let latest = 0;
  let running = false;
  let abandoned = false;
  let disposed = false;
  let staleTimer: ReturnType<typeof setTimeout> | undefined;
  let stopTimer: () => void = () => {};

  const schedule = (backoff: boolean): void => {
    stopTimer();
    stopTimer = backoff ? autoRetry(tick) : stopInterval(setInterval(tick, ms));
  };

  function tick(): void {
    if (disposed || document.hidden || running) {
      return;
    }

    latest += 1;
    const mine = latest;
    running = true;

    staleTimer = setTimeout(() => {
      if (mine === latest) {
        running = false;

        if (!abandoned) {
          abandoned = true;
          schedule(true);
        }
      }
    }, FLIGHT_STALE_MS);

    void settle(run).finally(() => {
      // A run that ends after the view was closed must not start a schedule nobody stops.
      if (disposed) {
        return;
      }

      // The late end of a run that was given up on says nothing about the one that replaced it.
      if (mine !== latest) {
        return;
      }

      clearTimeout(staleTimer);
      running = false;

      if (abandoned) {
        abandoned = false;
        schedule(failing);
      }
    });
  }

  schedule(failing);
  window.addEventListener('focus', tick);

  return () => {
    disposed = true;
    stopTimer();
    clearTimeout(staleTimer);
    window.removeEventListener('focus', tick);
  };
}

/** Runs `run` and waits for whatever it returns; a thrown error or a rejection only ends the run. */
async function settle(run: () => unknown): Promise<void> {
  try {
    await run();
  } catch {
    // The reader reports its own failures.
  }
}

function stopInterval(timer: ReturnType<typeof setInterval>): () => void {
  return () => clearInterval(timer);
}
