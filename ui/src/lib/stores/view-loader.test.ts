import { afterEach, describe, expect, it, vi } from 'vitest';

import { FLIGHT_STALE_MS } from './live';

import type { HarnessId } from '$lib/ipc/bindings/HarnessId';

import { ViewLoader } from './view-loader.svelte';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

interface Loaded {
  harness: HarnessId;
  project: string | null;
}

function recorder() {
  const calls: Loaded[] = [];
  const loader = new ViewLoader<Loaded>(async (harness, project) => {
    calls.push({ harness, project });

    return { harness, project };
  });

  return { loader, calls };
}

afterEach(() => vi.useRealTimers());

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('ViewLoader', () => {
  it('loads when the harness or the project changes, but not for the same pair', async () => {
    const { loader, calls } = recorder();

    loader.sync('omp', null);
    loader.sync('omp', null);
    await settle();
    loader.sync('omp', '/work/app');
    await settle();
    loader.sync('pi', '/work/app');
    await settle();

    expect(calls).toEqual([
      { harness: 'omp', project: null },
      { harness: 'omp', project: '/work/app' },
      { harness: 'pi', project: '/work/app' },
    ]);
    expect(loader.status).toBe('ready');
    expect(loader.data).toEqual({ harness: 'pi', project: '/work/app' });
  });

  it('reloads the current pair on refresh', async () => {
    const { loader, calls } = recorder();
    loader.sync('omp', null);
    await settle();

    loader.refresh();
    await settle();

    expect(calls).toHaveLength(2);
  });

  it('re-reads a pair it already holds on revalidate, without a loading state', async () => {
    const { loader, calls } = recorder();
    loader.revalidate('omp', '/work/app');
    await settle();

    loader.revalidate('omp', '/work/app');

    expect(loader.status).toBe('ready');
    await settle();
    expect(calls).toHaveLength(2);
    expect(loader.data).toEqual({ harness: 'omp', project: '/work/app' });
  });

  it('keeps what is shown when a quiet refresh fails', async () => {
    let fail = false;
    const loader = new ViewLoader<string>(async () => {
      if (fail) {
        throw new Error('disk busy');
      }

      return 'sessions';
    });
    loader.sync('omp', null);
    await settle();

    fail = true;
    await loader.refresh({ quiet: true });

    expect(loader.status).toBe('ready');
    expect(loader.data).toBe('sessions');
  });

  it('ignores a slow response for a pair that is no longer current', async () => {
    const slow = deferred<Loaded>();
    const loader = new ViewLoader<Loaded>((harness, project) => (harness === 'omp' ? slow.promise : Promise.resolve({ harness, project })));

    loader.sync('omp', null);
    loader.sync('pi', null);
    await settle();
    slow.resolve({ harness: 'omp', project: null });
    await settle();

    expect(loader.data?.harness).toBe('pi');
  });

  it('reports a failed load and recovers on refresh', async () => {
    let failing = true;
    const loader = new ViewLoader<Loaded>(async (harness, project) => {
      if (failing) {
        throw { code: 'io', message: 'unreadable' };
      }

      return { harness, project };
    });

    loader.sync('omp', null);
    await settle();
    expect(loader.status).toBe('error');
    expect(loader.error?.code).toBe('io');

    failing = false;
    loader.refresh();
    await settle();

    expect(loader.status).toBe('ready');
    expect(loader.error).toBeNull();
  });

  it('clears the view when there is no harness', async () => {
    const { loader } = recorder();
    loader.sync('omp', null);
    await settle();

    loader.sync(null, null);

    expect(loader.status).toBe('idle');
    expect(loader.data).toBeNull();
  });

  it('recovers from a failed load on a quiet refresh without showing the loading state again', async () => {
    let failing = true;
    const loader = new ViewLoader<string>(async () => {
      if (failing) {
        throw { code: 'io', message: 'unreadable' };
      }

      return 'sessions';
    });
    const seen: string[] = [];
    loader.sync('omp', null);
    await settle();

    await loader.refresh({ quiet: true });
    seen.push(loader.status);
    failing = false;
    const refreshing = loader.refresh({ quiet: true });
    seen.push(loader.status);
    await refreshing;
    seen.push(loader.status);

    expect(seen).toEqual(['error', 'error', 'ready']);
    expect(loader.data).toBe('sessions');
  });

  it('keeps the very same data on screen when a quiet refresh finds nothing new', async () => {
    const loader = new ViewLoader<{ items: string[] }>(async () => ({ items: ['a', 'b'] }));
    loader.sync('omp', null);
    await settle();
    const shown = loader.data;

    await loader.refresh({ quiet: true });

    expect(loader.data).toBe(shown);
  });

  it('replaces the data when a quiet refresh finds something new', async () => {
    let items = ['a'];
    const loader = new ViewLoader<{ items: string[] }>(async () => ({ items }));
    loader.sync('omp', null);
    await settle();

    items = ['a', 'b'];
    await loader.refresh({ quiet: true });

    expect(loader.data).toEqual({ items: ['a', 'b'] });
  });

  it('lets a first load that is still running finish before a quiet refresh reads, so a failing one cannot strand it on loading', async () => {
    const first = deferred<string>();
    let calls = 0;
    const loader = new ViewLoader<string>(() => {
      calls += 1;

      return calls === 1 ? first.promise : Promise.reject(new Error('disk busy'));
    });
    loader.sync('omp', null);

    const refreshing = loader.refresh({ quiet: true });
    expect(calls).toBe(1);
    expect(loader.status).toBe('loading');

    first.resolve('sessions');
    await refreshing;

    expect(calls).toBe(2);
    expect(loader.status).toBe('ready');
    expect(loader.data).toBe('sessions');
  });

  it('keeps the newest data when an older background read answers late', async () => {
    const slow = deferred<string>();
    let calls = 0;
    const loader = new ViewLoader<string>(() => {
      calls += 1;

      return calls === 2 ? slow.promise : Promise.resolve(`read ${calls}`);
    });
    loader.sync('omp', null);
    await settle();

    const older = loader.refresh({ quiet: true });
    await loader.refresh({ quiet: true });
    slow.resolve('read 2');
    await older;

    expect(loader.data).toBe('read 3');
  });

  it('shows the error when a quiet read fails and there is nothing on screen to keep', async () => {
    let reason = 'first';
    const loader = new ViewLoader<string>(async () => {
      throw { code: 'io', message: reason };
    });
    loader.sync('omp', null);
    await settle();

    reason = 'second';
    await loader.refresh({ quiet: true });

    expect(loader.status).toBe('error');
    expect(loader.error?.message).toBe('second');
  });

  it('stops waiting for a first load that never answers, and ignores its late answer once newer data is in', async () => {
    vi.useFakeTimers();
    const never = deferred<string>();
    let calls = 0;
    const loader = new ViewLoader<string>(() => {
      calls += 1;

      return calls === 1 ? never.promise : Promise.resolve('fresh');
    });
    loader.sync('omp', null);

    const refreshing = loader.refresh({ quiet: true });
    await vi.advanceTimersByTimeAsync(FLIGHT_STALE_MS);
    await refreshing;

    expect(loader.status).toBe('ready');
    expect(loader.data).toBe('fresh');

    never.resolve('stale');
    await vi.advanceTimersByTimeAsync(0);
    expect(loader.data).toBe('fresh');

    await loader.refresh({ quiet: true });
    expect(calls).toBe(3);
  });

  it('lets the answer of a newer load land when a background read was waiting on a load that was switched away from', async () => {
    const first = deferred<string>();
    const second = deferred<string>();
    const reads: string[] = [];
    const loader = new ViewLoader<string>((harness) => {
      reads.push(harness);

      return harness === 'omp' ? first.promise : second.promise;
    });
    loader.sync('omp', null);

    const refreshing = loader.refresh({ quiet: true });
    loader.sync('pi', null);
    first.resolve('omp data');
    await refreshing;

    expect(reads).toEqual(['omp', 'pi']);
    expect(loader.status).toBe('loading');

    second.resolve('pi data');
    await settle();

    expect(loader.status).toBe('ready');
    expect(loader.data).toBe('pi data');
  });
});
