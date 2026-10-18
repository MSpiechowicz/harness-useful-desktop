import { afterEach, describe, expect, it, vi } from 'vitest';

import { fakeTransport } from '../../../tests/helpers';

import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';

import { FLIGHT_STALE_MS } from './live';
import { EcosystemStore, isHigherVersion, withReleases } from './ecosystem.svelte';

function item(id: EcosystemItem['id'], version: string | null, latestVersion: string | null = null): EcosystemItem {
  return {
    id,
    name: id,
    tagline: '',
    description: '',
    repoUrl: '',
    docsUrl: '',
    installed: version !== null,
    version,
    latestVersion,
    updateAvailable: latestVersion !== null,
    via: [],
    supportedOnThisOs: true,
    installSteps: [],
    notes: [],
  };
}

const status: EcosystemStatus = { items: [item('dashboard', '1.22.1'), item('skills', '0.2.36'), item('sidebar', null)] };

describe('withReleases', () => {
  it('marks a companion whose GitHub release is newer and names that version', () => {
    const merged = withReleases(status, [{ id: 'dashboard', latestVersion: '1.25.0', updateAvailable: true }]);

    expect(merged.items[0]).toMatchObject({ updateAvailable: true, latestVersion: '1.25.0' });
    expect(merged.items[1]).toBe(status.items[1]);
  });

  it('leaves a companion that is current, or that GitHub said nothing about, as detected', () => {
    const catalog: EcosystemStatus = { items: [item('skills', '0.2.36', '0.2.38')] };
    const merged = withReleases(catalog, [{ id: 'skills', latestVersion: '0.2.36', updateAvailable: false }]);

    expect(merged).toBe(catalog);
    expect(merged.items[0]).toMatchObject({ updateAvailable: true, latestVersion: '0.2.38' });
  });

  it('names the higher version when the catalog is ahead of GitHub', () => {
    const catalog: EcosystemStatus = { items: [item('skills', '0.2.36', '0.2.50')] };
    const merged = withReleases(catalog, [{ id: 'skills', latestVersion: '0.2.40', updateAvailable: true }]);

    expect(merged.items[0]).toMatchObject({ updateAvailable: true, latestVersion: '0.2.50' });
  });

  it('keeps the status as is when there are no releases', () => {
    expect(withReleases(status, [])).toBe(status);
  });
});

describe('isHigherVersion', () => {
  it('compares numeric parts, a missing part counting as zero', () => {
    expect(isHigherVersion('1.10.0', '1.9.9')).toBe(true);
    expect(isHigherVersion('v2', '1.9.9')).toBe(true);
    expect(isHigherVersion('1.0', '1.0.0')).toBe(false);
    expect(isHigherVersion('1.2.0', '1.3.0')).toBe(false);
  });

  it('never ranks a version with letters higher', () => {
    expect(isHigherVersion('1.0.0.beta', '1.0.0')).toBe(false);
    expect(isHigherVersion('1.0.1', 'dev')).toBe(false);
  });
});

describe('a quiet read of the ecosystem', () => {
  it('leaves a failure on screen instead of flashing the loading state, and recovers when the lookup works', async () => {
    let failing = true;
    const store = new EcosystemStore(
      fakeTransport({
        ecosystem_status: () => {
          if (failing) {
            throw { code: 'internal', message: 'no detection' };
          }

          return status;
        },
        ecosystem_releases: () => [],
      }),
    );
    await store.load();
    expect(store.state).toBe('error');

    await store.load(true, { quiet: true });
    expect(store.state).toBe('error');

    failing = false;
    const reading = store.load(true, { quiet: true });
    expect(store.state).toBe('error');
    await reading;

    expect(store.state).toBe('ready');
    expect(store.status).toEqual(status);
  });
});

describe('ecosystem reads in flight', () => {
  it('does not start another detection while one is running, for a poll or for a view that opens meanwhile', async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    let reads = 0;
    const store = new EcosystemStore(
      fakeTransport({
        ecosystem_status: async () => {
          reads += 1;
          await gate;

          return status;
        },
        ecosystem_releases: () => [],
      }),
    );

    const first = store.load();
    const polled = store.load(false, { quiet: true });
    const forcedPoll = store.load(true, { quiet: true });
    const opened = store.load();
    release();
    await Promise.all([first, polled, forcedPoll, opened]);

    expect(reads).toBe(1);
    expect(store.state).toBe('ready');
  });

  it('lets the user force a new detection past a running one', async () => {
    let reads = 0;
    const store = new EcosystemStore(
      fakeTransport({
        ecosystem_status: async () => {
          reads += 1;
          await new Promise((resolve) => setTimeout(resolve, 5));

          return status;
        },
        ecosystem_releases: () => [],
      }),
    );

    const first = store.load();
    const forced = store.load(true);
    await Promise.all([first, forced]);

    expect(reads).toBe(2);
  });

  it('keeps the reading on screen when a quiet read fails afterwards', async () => {
    let failing = false;
    const store = new EcosystemStore(
      fakeTransport({
        ecosystem_status: () => {
          if (failing) {
            throw { code: 'internal', message: 'no detection' };
          }

          return status;
        },
        ecosystem_releases: () => [],
      }),
    );
    await store.load();

    failing = true;
    await store.load(true, { quiet: true });

    expect(store.state).toBe('ready');
    expect(store.status).toEqual(status);
  });
});

describe('quiet reads that find nothing new', () => {
  afterEach(() => vi.useRealTimers());

  it('keep the update badge and the reading itself on screen while the release lookup runs again', async () => {
    let lookups = 0;
    let finishLookup!: () => void;
    const store = new EcosystemStore(
      fakeTransport({
        ecosystem_status: () => structuredClone(status),
        ecosystem_releases: async () => {
          lookups += 1;

          if (lookups > 1) {
            await new Promise<void>((resolve) => (finishLookup = resolve));
          }

          return [{ id: 'dashboard', latestVersion: '1.25.0', updateAvailable: true }];
        },
      }),
    );
    await store.loadWithReleases();
    const shown = store.status;
    expect(store.hasUpdate('dashboard')).toBe(true);

    await store.load(true, { quiet: true });

    expect(store.hasUpdate('dashboard')).toBe(true);
    expect(store.releasesPending).toBe(false);
    expect(store.status).toBe(shown);

    finishLookup();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(store.status).toBe(shown);
  });

  it('show a companion that changed on disk without its stale update badge', async () => {
    let installed = '1.22.1';
    const store = new EcosystemStore(
      fakeTransport({
        ecosystem_status: () => ({ items: [item('dashboard', installed)] }),
        ecosystem_releases: () => [{ id: 'dashboard', latestVersion: '1.25.0', updateAvailable: installed !== '1.25.0' }],
      }),
    );
    await store.loadWithReleases();
    expect(store.hasUpdate('dashboard')).toBe(true);

    installed = '1.25.0';
    await store.loadWithReleases(true);

    expect(store.status?.items[0]).toMatchObject({ version: '1.25.0', updateAvailable: false });
  });

  it('end the loading state of a forced read that was given up on, even when the quiet read that follows fails', async () => {
    vi.useFakeTimers();
    let reads = 0;
    const store = new EcosystemStore(
      fakeTransport({
        ecosystem_status: () => {
          reads += 1;

          if (reads === 2) {
            return new Promise(() => {});
          }

          if (reads === 3) {
            throw { code: 'internal', message: 'no detection' };
          }

          return status;
        },
        ecosystem_releases: () => [],
      }),
    );
    await store.load();

    void store.load(true);
    expect(store.state).toBe('loading');

    await vi.advanceTimersByTimeAsync(FLIGHT_STALE_MS);
    await store.load(true, { quiet: true });

    expect(store.state).toBe('ready');
    expect(store.status).toEqual(status);
  });
});
