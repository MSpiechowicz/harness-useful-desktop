import { afterEach, describe, expect, it, vi } from 'vitest';

import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessRelease } from '$lib/ipc/bindings/HarnessRelease';
import type { HarnessUpdateResult } from '$lib/ipc/bindings/HarnessUpdateResult';
import type { CommandName } from '$lib/ipc/commands';
import { createMockTransport } from '$lib/ipc/mock/transport';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { HarnessStore } from './harness.svelte';
import { HarnessUpdatesStore } from './harness-updates.svelte';
import { PrefsStore } from './prefs.svelte';
import { ToastsStore } from './toasts.svelte';

type Replies = Partial<Record<CommandName, (args: never) => unknown>>;

/** A reply that waits until the test lets it through. */
function held<T>() {
  let release: (value: T) => void = () => {};
  let fail: (reason: unknown) => void = () => {};
  const promise = new Promise<T>((resolve, reject) => {
    release = resolve;
    fail = reject;
  });

  return { promise, release, fail };
}

function releaseOf(harness: HarnessId, latestVersion: string, updateAvailable = true): HarnessRelease {
  return { harness, latestVersion, updateAvailable, updateCommand: `${harness} update` };
}

/** The mock machine (OMP 18.6.1 in use, OpenCode 1.19.3 beside it), where a test can answer a command itself. */
async function setup(replies: Replies = {}) {
  const mock = createMockTransport(defaultScenario(), 0);
  const transport: Transport = {
    invoke: (name, args) => (replies[name] ? (replies[name](args as never) as never) : mock.invoke(name, args)),
  };
  const toasts = new ToastsStore();
  const prefs = new PrefsStore(transport, toasts);
  await prefs.load();
  prefs.value = { ...prefs.value!, defaultHarness: 'omp' };

  const harness = new HarnessStore(transport, prefs, toasts);
  await harness.load();

  return { store: new HarnessUpdatesStore(transport, harness, toasts), harness, prefs, toasts };
}

describe('HarnessUpdatesStore', () => {
  afterEach(() => vi.unstubAllGlobals());

  describe('the offer', () => {
    it('names the newer version of an installed harness, and nothing for one that is current', async () => {
      const { store } = await setup({ harness_releases: () => [releaseOf('omp', '18.7.0'), releaseOf('opencode', '1.19.3', false)] });

      await store.refresh();

      expect(store.offer('omp')?.latestVersion).toBe('18.7.0');
      expect(store.offer('opencode')).toBeUndefined();
    });

    it('goes away when the lookup answers with nothing, as it does with the setting off', async () => {
      const state = { off: false };
      const { store } = await setup({ harness_releases: () => (state.off ? [] : [releaseOf('omp', '18.7.0')]) });
      await store.refresh();
      expect(store.offer('omp')).toBeDefined();

      state.off = true;
      await store.refresh();

      expect(store.offer('omp')).toBeUndefined();
    });

    it('goes away when the harness is updated elsewhere, before any new lookup', async () => {
      const { store, harness } = await setup({ harness_releases: () => [releaseOf('omp', '18.7.0')] });
      await store.refresh();

      harness.summaries = harness.summaries.map((summary) => (summary.info.id === 'omp' ? { ...summary, detection: { ...summary.detection, version: '18.7.0' } } : summary));

      expect(store.offer('omp')).toBeUndefined();
    });

    it('is only sent for harnesses with a known version', async () => {
      const asked: unknown[] = [];
      const { store, harness } = await setup({
        harness_releases: (args: { installed: unknown[] }) => {
          asked.push(args.installed);

          return [];
        },
      });
      harness.summaries = harness.summaries.map((summary) => (summary.info.id === 'opencode' ? { ...summary, detection: { ...summary.detection, version: null } } : summary));

      await store.refresh();

      expect(asked).toEqual([[{ harness: 'omp', version: '18.6.1' }]]);
    });

    it('keeps what is shown when a lookup fails, without a message', async () => {
      const state = { failing: false };
      const { store, toasts } = await setup({
        harness_releases: () => {
          if (state.failing) {
            throw new IpcFailure({ code: 'internal', message: 'offline' });
          }

          return [releaseOf('omp', '18.7.0')];
        },
      });
      await store.refresh();

      state.failing = true;
      await store.refresh();

      expect(store.offer('omp')).toBeDefined();
      expect(toasts.items).toHaveLength(0);
    });

    it('is answered by the lookup that ran last when one is asked for during another', async () => {
      const first = held<HarnessRelease[]>();
      let lookups = 0;
      const { store } = await setup({
        harness_releases: () => {
          lookups += 1;

          return lookups === 1 ? first.promise : [];
        },
      });

      const running = store.refresh();
      const joined = store.refresh();
      first.release([releaseOf('omp', '18.7.0')]);
      await Promise.all([running, joined]);

      expect(lookups).toBe(2);
      expect(store.offer('omp')).toBeUndefined();
    });
  });

  describe('the suggestion', () => {
    it('raises one sticky toast for the harness in use, and none again in the run whatever happens to it', async () => {
      const state = { latest: '18.7.0' };
      const { store, toasts } = await setup({ harness_releases: () => [releaseOf('omp', state.latest)] });

      await store.suggest('omp');

      expect(toasts.items).toHaveLength(1);
      expect(toasts.items[0]).toMatchObject({ tone: 'neutral', duration: 0 });
      expect(toasts.items[0].title).toContain('18.7.0');
      expect(toasts.items[0].action).toBeDefined();

      toasts.dismiss(toasts.items[0].id);
      await store.suggest('omp');
      state.latest = '18.8.0';
      await store.suggest('omp');

      expect(toasts.items).toHaveLength(0);
    });

    it('raises none for a harness that is not the one in use', async () => {
      const { store, toasts } = await setup({ harness_releases: () => [releaseOf('omp', '18.7.0'), releaseOf('opencode', '1.20.0')] });

      await store.suggest('opencode');

      expect(toasts.items).toHaveLength(0);
      expect(store.offer('opencode')).toBeDefined();
    });

    it('raises none when there is nothing newer', async () => {
      const { store, toasts } = await setup({ harness_releases: () => [releaseOf('omp', '18.6.1', false)] });

      await store.suggest('omp');

      expect(toasts.items).toHaveLength(0);
    });

    it('raises none when the user moved to another harness while the lookup ran, and still can for the one moved to', async () => {
      const lookup = held<HarnessRelease[]>();
      const { store, prefs, toasts } = await setup({ harness_releases: () => lookup.promise });

      const suggesting = store.suggest('omp');
      prefs.value = { ...prefs.value!, defaultHarness: 'opencode' };
      lookup.release([releaseOf('omp', '18.7.0'), releaseOf('opencode', '1.20.0')]);
      await suggesting;

      expect(toasts.items).toHaveLength(0);

      await store.suggest('opencode');

      expect(toasts.items).toHaveLength(1);
    });
  });

  describe('the update', () => {
    async function offered(replies: Replies = {}) {
      const machine = await setup({ harness_releases: () => [releaseOf('omp', '18.7.0')], ...replies });
      await machine.store.suggest('omp');

      return machine;
    }

    it('shows its progress in a toast, then the new version in the same toast, and takes the new version in', async () => {
      const { store, harness, toasts } = await offered();
      const [offer] = toasts.items;

      toasts.dismiss(offer.id);
      const updating = store.update('omp');

      expect(store.isUpdating('omp')).toBe(true);
      expect(toasts.items).toHaveLength(1);
      expect(toasts.items[0]).toMatchObject({ duration: 0, action: undefined });
      const progress = toasts.items[0].id;

      await updating;

      expect(store.isUpdating('omp')).toBe(false);
      expect(harness.get('omp')?.detection.version).toBe('18.7.0');
      expect(store.offer('omp')).toBeUndefined();
      expect(toasts.items).toHaveLength(1);
      expect(toasts.items[0]).toMatchObject({ id: progress, tone: 'success', action: undefined });
      expect(toasts.items[0].title).toContain('18.7.0');
    });

    it('is started by the action of the offer toast', async () => {
      const { harness, toasts } = await offered();

      toasts.items[0].action?.run();

      await vi.waitFor(() => expect(harness.get('omp')?.detection.version).toBe('18.7.0'));
    });

    it('takes the offer toast away when it starts from somewhere else, so the toast cannot run it twice', async () => {
      const { store, toasts } = await offered();
      const [offer] = toasts.items;

      await store.update('omp');

      expect(toasts.items.map((toast) => toast.id)).not.toContain(offer.id);
      expect(toasts.items).toHaveLength(1);
    });

    it('takes the offer toast away when the harness was updated elsewhere', async () => {
      const npm = { behind: true };
      const { store, harness, toasts } = await offered({ harness_releases: () => [releaseOf('omp', '18.7.0', npm.behind)] });
      const current = harness.get('omp')!;

      npm.behind = false;
      harness.adoptUpdated({ ...current, detection: { ...current.detection, version: '18.7.0' } });
      await store.refresh();

      expect(toasts.items).toHaveLength(0);
    });

    it('says why nothing was updated and offers the command to copy', async () => {
      const writeText = vi.fn(async () => {});
      vi.stubGlobal('navigator', { clipboard: { writeText } });
      const current: { harness?: HarnessStore } = {};
      const machine = await offered({
        harness_update: () => ({ updated: false, summary: current.harness?.get('omp'), message: 'EACCES: permission denied', command: 'omp update' }),
      });
      current.harness = machine.harness;

      await machine.store.update('omp');

      expect(machine.toasts.items.at(-1)).toMatchObject({ tone: 'danger', description: 'EACCES: permission denied', duration: 0 });
      expect(machine.harness.get('omp')?.detection.version).toBe('18.6.1');

      machine.toasts.items.at(-1)?.action?.run();

      expect(writeText).toHaveBeenCalledWith('omp update');
    });

    it('reports a failed call with its message and the command of the release', async () => {
      const writeText = vi.fn(async () => {});
      vi.stubGlobal('navigator', { clipboard: { writeText } });
      const { store, toasts } = await offered({
        harness_update: () => {
          throw new IpcFailure({ code: 'invalidInput', message: 'omp is already being updated' });
        },
      });

      await store.update('omp');

      expect(toasts.items.at(-1)).toMatchObject({ tone: 'danger', description: 'omp is already being updated' });
      expect(store.isUpdating('omp')).toBe(false);

      toasts.items.at(-1)?.action?.run();

      expect(writeText).toHaveBeenCalledWith('omp update');
    });

    it('refuses a second run of the same harness while one is running', async () => {
      const run = held<HarnessUpdateResult>();
      let runs = 0;
      const { store, harness, toasts } = await offered({
        harness_update: () => {
          runs += 1;

          return run.promise;
        },
      });
      toasts.dismiss(toasts.items[0].id);

      const first = store.update('omp');
      const second = store.update('omp');
      await second;

      expect(runs).toBe(1);
      expect(toasts.items).toHaveLength(1);
      expect(store.isUpdating('omp')).toBe(true);

      run.release({ updated: false, summary: harness.get('omp')!, message: null, command: 'omp update' });
      await first;

      expect(store.isUpdating('omp')).toBe(false);
    });

    describe('when another harness is updating', () => {
      async function twoHarnesses(replies: Replies = {}) {
        return setup({ harness_releases: () => [releaseOf('omp', '18.7.0'), releaseOf('opencode', '1.20.0')], ...replies });
      }

      it('waits for it, then runs and reports its own outcome', async () => {
        const first = held<HarnessUpdateResult>();
        const calls: unknown[] = [];
        const current: { harness?: HarnessStore } = {};
        const { store, harness, toasts } = await twoHarnesses({
          harness_update: (args: { harness: HarnessId }) => {
            calls.push(args.harness);

            return args.harness === 'omp'
              ? first.promise
              : { updated: true, summary: { ...current.harness!.get('opencode')!, detection: { ...current.harness!.get('opencode')!.detection, version: '1.20.0' } }, message: null, command: 'opencode update' };
          },
        });
        current.harness = harness;
        await store.refresh();

        const running = store.update('omp');
        const queued = store.update('opencode');

        expect(store.isUpdating('opencode')).toBe(true);
        expect(toasts.items).toHaveLength(2);
        await vi.waitFor(() => expect(calls).toEqual(['omp']));

        first.release({ updated: true, summary: harness.get('omp')!, message: null, command: 'omp update' });
        await Promise.all([running, queued]);

        expect(calls).toEqual(['omp', 'opencode']);
        expect(store.isUpdating('opencode')).toBe(false);
        expect(harness.get('opencode')?.detection.version).toBe('1.20.0');
        expect(toasts.items.at(-1)).toMatchObject({ tone: 'success' });
        expect(toasts.items.at(-1)?.title).toContain('1.20.0');
      });

      it('still runs after the first update failed', async () => {
        const first = held<HarnessUpdateResult>();
        const calls: unknown[] = [];
        const { store, toasts } = await twoHarnesses({
          harness_update: (args: { harness: HarnessId }) => {
            calls.push(args.harness);

            if (args.harness === 'omp') {
              return first.promise;
            }

            throw new IpcFailure({ code: 'internal', message: 'opencode failed too' });
          },
        });
        await store.refresh();

        const running = store.update('omp');
        const queued = store.update('opencode');
        first.fail(new IpcFailure({ code: 'internal', message: 'omp failed' }));
        await Promise.all([running, queued]);

        expect(calls).toEqual(['omp', 'opencode']);
        expect(store.isUpdating('omp')).toBe(false);
        expect(store.isUpdating('opencode')).toBe(false);
        expect(toasts.items.map((toast) => toast.description)).toEqual(['omp failed', 'opencode failed too']);
      });
    });
  });
});
