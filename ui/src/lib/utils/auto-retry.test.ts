import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { RETRY_BACKOFF_MS } from '$lib/stores/live';

import { autoRetry } from './auto-retry';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('autoRetry', () => {
  it('tries again after a longer wait each time, and keeps the longest wait', () => {
    const run = vi.fn();
    const stop = autoRetry(run);

    for (let attempt = 0; attempt < RETRY_BACKOFF_MS.length + 2; attempt += 1) {
      const wait = RETRY_BACKOFF_MS[Math.min(attempt, RETRY_BACKOFF_MS.length - 1)];
      const before = run.mock.calls.length;

      vi.advanceTimersByTime(wait - 1);
      expect(run).toHaveBeenCalledTimes(before);

      vi.advanceTimersByTime(1);
      expect(run).toHaveBeenCalledTimes(before + 1);
    }

    stop();
    expect(RETRY_BACKOFF_MS).toEqual([...RETRY_BACKOFF_MS].sort((a, b) => a - b));
  });

  it('stops for good once the failure is gone', () => {
    const run = vi.fn();
    const stop = autoRetry(run);

    vi.advanceTimersByTime(RETRY_BACKOFF_MS[0]);
    stop();
    vi.advanceTimersByTime(10 * RETRY_BACKOFF_MS.at(-1)!);

    expect(run).toHaveBeenCalledTimes(1);
  });

  it('skips the tries made while the window is hidden but goes on counting', () => {
    const run = vi.fn();
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    const stop = autoRetry(run, [100, 200]);

    vi.advanceTimersByTime(100);
    expect(run).not.toHaveBeenCalled();

    hidden.mockReturnValue(false);
    vi.advanceTimersByTime(200);
    expect(run).toHaveBeenCalledTimes(1);

    stop();
  });
});
