import { describe, expect, it } from 'vitest';

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
});
