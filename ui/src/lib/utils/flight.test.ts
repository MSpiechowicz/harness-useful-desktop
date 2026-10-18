import { afterEach, describe, expect, it, vi } from 'vitest';

import { FLIGHT_STALE_MS } from '$lib/stores/live';

import { Flight } from './flight';

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((res) => (resolve = res));

  return { promise, resolve };
}

afterEach(() => vi.useRealTimers());

describe('Flight', () => {
  it('skips a second read while the first one is running, and reads again once it finished', async () => {
    const flight = new Flight();
    const first = deferred();
    const task = vi.fn(() => first.promise);

    const started = flight.startUnlessRunning(task);
    const skipped = flight.startUnlessRunning(task);
    expect(task).toHaveBeenCalledTimes(1);
    expect(skipped).toBe(started);
    expect(flight.running).toBe(started);

    first.resolve();
    await started;
    expect(flight.running).toBeNull();

    await flight.startUnlessRunning(async () => undefined);
    expect(task).toHaveBeenCalledTimes(1);
  });

  it('lets a newer read replace the one in flight, and the older one finishing does not clear it', async () => {
    const flight = new Flight();
    const older = deferred();
    const newer = deferred();

    const first = flight.start(() => older.promise);
    const second = flight.start(() => newer.promise);
    older.resolve();
    await first;

    expect(flight.running).toBe(second);

    newer.resolve();
    await second;
    expect(flight.running).toBeNull();
  });

  it('gives up on a read that never answers, so the next one starts, and its late answer does not clear the newer read', async () => {
    vi.useFakeTimers();
    const flight = new Flight();
    const never = deferred();
    const newer = deferred();
    const task = vi.fn(() => newer.promise);

    const stuck = flight.startUnlessRunning(() => never.promise);
    await vi.advanceTimersByTimeAsync(FLIGHT_STALE_MS - 1);
    expect(flight.startUnlessRunning(task)).toBe(stuck);
    expect(task).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(flight.running).toBeNull();

    const next = flight.startUnlessRunning(task);
    expect(task).toHaveBeenCalledTimes(1);

    never.resolve();
    await stuck;
    expect(flight.running).toBe(next);

    newer.resolve();
    await next;
    expect(flight.running).toBeNull();
  });
});
