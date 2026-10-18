import { RETRY_BACKOFF_MS } from '$lib/stores/live';

/**
 * Calls `run` again and again while something stays failed, each time after a longer wait (the last wait repeats), so a
 * view that could not load recovers by itself without the user pressing Try again. `run` should be quiet: no spinner
 * and no message of its own, since the failure is already on screen. A tick while the window is hidden is skipped.
 * Start it from an `$effect` that only runs while the failure shows; returns the cleanup.
 */
export function autoRetry(run: () => void, waits: readonly number[] = RETRY_BACKOFF_MS): () => void {
  let timer: ReturnType<typeof setTimeout>;
  let attempt = 0;

  const schedule = (): void => {
    const wait = waits[Math.min(attempt, waits.length - 1)];

    timer = setTimeout(() => {
      attempt += 1;

      if (!document.hidden) {
        run();
      }

      schedule();
    }, wait);
  };

  schedule();

  return () => clearTimeout(timer);
}
