import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { FLIGHT_STALE_MS, RETRY_BACKOFF_MS } from '$lib/stores/live';

import { autoRefresh } from './auto-refresh';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('autoRefresh', () => {
  it('runs every interval and again when the window regains focus', async () => {
    const run = vi.fn();
    const stop = autoRefresh(run, 1000);

    await vi.advanceTimersByTimeAsync(2500);
    expect(run).toHaveBeenCalledTimes(2);

    window.dispatchEvent(new Event('focus'));
    expect(run).toHaveBeenCalledTimes(3);

    stop();
    await vi.advanceTimersByTimeAsync(5000);
    window.dispatchEvent(new Event('focus'));
    expect(run).toHaveBeenCalledTimes(3);
  });

  it('skips a tick while the previous run is still going, and runs again once it ended', async () => {
    let finish!: () => void;
    const run = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    const stop = autoRefresh(run, 1000);

    await vi.advanceTimersByTimeAsync(3500);
    expect(run).toHaveBeenCalledTimes(1);

    finish();
    await vi.advanceTimersByTimeAsync(1000);
    expect(run).toHaveBeenCalledTimes(2);

    stop();
  });

  it('keeps going after a run that threw', async () => {
    const run = vi.fn().mockRejectedValue(new Error('disk busy'));
    const stop = autoRefresh(run, 1000);

    await vi.advanceTimersByTimeAsync(2500);

    expect(run).toHaveBeenCalledTimes(2);
    stop();
  });

  it('does not run while the window is hidden', () => {
    const run = vi.fn();
    vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    const stop = autoRefresh(run, 1000);

    vi.advanceTimersByTime(3000);

    expect(run).not.toHaveBeenCalled();
    stop();
  });

  it('follows the retry schedule instead of the interval while failing', async () => {
    const run = vi.fn();
    const stop = autoRefresh(run, 60_000, { failing: true });

    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0] - 1);
    expect(run).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(run).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[1]);
    expect(run).toHaveBeenCalledTimes(2);

    stop();
  });

  it('gives up on a run that never settles, lets the next tick through, and then backs off like a failure', async () => {
    const run = vi.fn(() => new Promise<void>(() => {}));
    const stop = autoRefresh(run, 1000);

    // The first run starts at the first tick, a second in.
    await vi.advanceTimersByTimeAsync(1000 + FLIGHT_STALE_MS - 1);
    expect(run).toHaveBeenCalledTimes(1);

    // Given up on: the retry schedule takes over from the steady pace.
    await vi.advanceTimersByTimeAsync(1);
    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0]);
    expect(run).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[1] - 1);
    expect(run).toHaveBeenCalledTimes(2);

    stop();
  });

  it('returns to the steady pace once a run settles again after one was given up on', async () => {
    let hang = true;
    const run = vi.fn(() => (hang ? new Promise<void>(() => {}) : Promise.resolve()));
    const stop = autoRefresh(run, 1000);

    await vi.advanceTimersByTimeAsync(1000 + FLIGHT_STALE_MS);
    hang = false;
    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0]);
    const afterRecovery = run.mock.calls.length;

    await vi.advanceTimersByTimeAsync(3500);

    expect(run.mock.calls.length).toBe(afterRecovery + 3);
    stop();
  });

  it('starts nothing when a run that was given up on settles after the view was closed', async () => {
    let finish!: () => void;
    const run = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    const stop = autoRefresh(run, 1000);

    await vi.advanceTimersByTimeAsync(1000 + FLIGHT_STALE_MS);
    stop();
    finish();
    await vi.advanceTimersByTimeAsync(0);
    const afterClose = run.mock.calls.length;

    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0] + 10_000);
    window.dispatchEvent(new Event('focus'));

    expect(run).toHaveBeenCalledTimes(afterClose);
  });
});
