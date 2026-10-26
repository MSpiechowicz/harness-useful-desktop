import { afterEach, describe, expect, it, vi } from 'vitest';

import { fakeTransport, samplePrefs, withoutChat } from '../../../tests/helpers';
import { withOpenCode2 } from '../ipc/mock/opencode-v2';
import { createMockTransport } from '../ipc/mock/transport';
import { defaultScenario } from '../ipc/mock/scenarios';

import { HarnessStore } from './harness.svelte';
import type { CommandName } from '../ipc/commands';
import type { Transport } from '../ipc/transport';
import { PrefsStore } from './prefs.svelte';
import { ToastsStore } from './toasts.svelte';

async function setup(prefsOverride: Parameters<typeof samplePrefs>[0] = {}) {
  const transport = createMockTransport(defaultScenario(), 0);
  const toasts = new ToastsStore();
  const prefs = new PrefsStore(transport, toasts);
  await prefs.load();
  prefs.value = { ...prefs.value!, ...prefsOverride };

  const store = new HarnessStore(transport, prefs, toasts);
  await store.load();
  // OpenCode stands in for a harness the app cannot chat with.
  withoutChat(store, 'opencode');

  return { store, prefs, toasts };
}

describe('HarnessStore', () => {
  it('tells the OpenCode generation from the detected version', async () => {
    const { store } = await setup();

    expect(store.openCodeGeneration).toBe(1);

    store.summaries = store.summaries.map(withOpenCode2);

    expect(store.openCodeGeneration).toBe(2);
  });

  it('falls back to the first installed harness when no daily driver is set', async () => {
    const { store } = await setup({ defaultHarness: null });

    expect(store.active?.info.id).toBe('omp');
  });

  it('keeps the chosen daily driver even when it is not installed', async () => {
    const { store } = await setup({ defaultHarness: 'pi' });

    expect(store.active?.info.id).toBe('pi');
    expect(store.active?.detection.installed).toBe(false);
  });

  it('refuses to select a harness that is not installed', async () => {
    const { store, prefs } = await setup({ defaultHarness: 'omp' });

    await store.select('pi');

    expect(prefs.value?.defaultHarness).toBe('omp');
  });

  it('stores the daily driver in the preferences when an installed harness is selected', async () => {
    const { store, prefs } = await setup({ defaultHarness: null });
    await store.pickBinary('pi');

    await store.select('pi');

    expect(prefs.value?.defaultHarness).toBe('pi');
    expect(store.active?.info.id).toBe('pi');
  });

  it('adopts a picked binary and offers a reset until it is cleared', async () => {
    const { store } = await setup();

    expect(store.hasOverride('pi')).toBe(false);

    await expect(store.pickBinary('pi')).resolves.toBe(true);
    expect(store.get('pi')?.detection.installed).toBe(true);
    expect(store.hasOverride('pi')).toBe(true);

    await store.clearBinary('pi');
    expect(store.get('pi')?.detection.installed).toBe(false);
    expect(store.hasOverride('pi')).toBe(false);
  });

  it('reports a cancelled file dialog as not picked', async () => {
    const toasts = new ToastsStore();
    const transport = fakeTransport({ harness_list: () => [], harness_pick_binary: () => null });
    const store = new HarnessStore(transport, new PrefsStore(transport, toasts), toasts);

    await expect(store.pickBinary('omp')).resolves.toBe(false);
    expect(toasts.items).toHaveLength(0);
  });

  it('shows a toast when the chosen file is refused', async () => {
    const toasts = new ToastsStore();
    const transport = fakeTransport({
      harness_pick_binary: () => {
        throw { code: 'invalidInput', message: 'That file is not the omp executable.' };
      },
    });
    const store = new HarnessStore(transport, new PrefsStore(transport, toasts), toasts);

    await expect(store.pickBinary('omp')).resolves.toBe(false);
    expect(toasts.items[0]).toMatchObject({ tone: 'danger', description: 'That file is not the omp executable.' });
  });

  describe('detection in the background', () => {
    afterEach(() => vi.useRealTimers());

    /** The mock machine, where PI turns up (or detection breaks) once the test says so. */
    async function machine() {
      const mock = createMockTransport(defaultScenario(), 0);
      const state = { piInstalled: false, failing: false };
      const transport: Transport = {
        async invoke(name: CommandName, args: never) {
          if (name === 'harness_detect' && state.failing) {
            throw new Error('scan failed');
          }

          const reply = (await mock.invoke(name, args)) as never as { info?: { id: string }; detection?: object };
          const turnsUp = name === 'harness_detect' && reply.info?.id === 'pi' && state.piInstalled;

          return (turnsUp ? { ...reply, detection: { ...reply.detection, installed: true } } : reply) as never;
        },
      };
      const toasts = new ToastsStore();
      const prefs = new PrefsStore(transport, toasts);
      await prefs.load();
      const store = new HarnessStore(transport, prefs, toasts);
      await store.load();

      return { store, state, toasts };
    }

    it('finds a harness installed since the last look, without a spinner or a message', async () => {
      const { store, state, toasts } = await machine();
      expect(store.get('pi')?.detection.installed).toBe(false);

      state.piInstalled = true;
      await store.rescan();

      expect(store.get('pi')?.detection.installed).toBe(true);
      expect(store.summaries).toHaveLength(5);
      expect(toasts.items).toEqual([]);
    });

    it('keeps a binary the user picked while a scan was running, instead of the older answer of that scan', async () => {
      const mock = createMockTransport(defaultScenario(), 0);
      let hold = false;
      let release!: () => void;
      const gate = new Promise<void>((resolve) => (release = resolve));
      const transport: Transport = {
        async invoke(name: CommandName, args: never) {
          const reply = await mock.invoke(name, args);

          if (name === 'harness_detect' && hold) {
            await gate;
          }

          return reply as never;
        },
      };
      const toasts = new ToastsStore();
      const store = new HarnessStore(transport, new PrefsStore(transport, toasts), toasts);
      await store.load();
      expect(store.get('pi')?.detection.installed).toBe(false);

      hold = true;
      const scanning = store.rescan();
      await store.pickBinary('pi');
      expect(store.get('pi')?.detection.installed).toBe(true);

      release();
      await scanning;

      expect(store.get('pi')?.detection.installed).toBe(true);
    });

    it('does not start another scan while one is running', async () => {
      const mock = createMockTransport(defaultScenario(), 0);
      let detects = 0;
      let release!: () => void;
      const gate = new Promise<void>((resolve) => (release = resolve));
      const transport: Transport = {
        async invoke(name: CommandName, args: never) {
          if (name === 'harness_detect') {
            detects += 1;
            await gate;
          }

          return mock.invoke(name, args);
        },
      };
      const toasts = new ToastsStore();
      const store = new HarnessStore(transport, new PrefsStore(transport, toasts), toasts);
      await store.load();

      const scans = [store.rescan(), store.rescan(), store.rescan()];
      release();
      await Promise.all(scans);

      expect(detects).toBe(store.summaries.length);
    });

    it('keeps what is shown, and says nothing, when a scan fails', async () => {
      const { store, state, toasts } = await machine();
      const before = store.summaries;

      state.failing = true;
      await store.rescan();

      expect(store.summaries).toBe(before);
      expect(toasts.items).toEqual([]);
    });

    it('does not replace the list when nothing changed', async () => {
      const { store } = await machine();
      const before = store.summaries;

      await store.rescan();

      expect(store.summaries).toBe(before);
    });

    it('looks again on coming back to the window, but at most once in the given gap', async () => {
      // Only the clock: the mock transport answers on a timer of its own.
      vi.useFakeTimers({ toFake: ['Date'] });
      const { store, state } = await machine();

      state.piInstalled = true;
      await store.rescanIfStale(10_000);
      expect(store.get('pi')?.detection.installed).toBe(false);

      vi.advanceTimersByTime(10_000);
      await store.rescanIfStale(10_000);
      expect(store.get('pi')?.detection.installed).toBe(true);

      state.piInstalled = false;
      vi.advanceTimersByTime(9_000);
      await store.rescanIfStale(10_000);
      expect(store.get('pi')?.detection.installed).toBe(true);
    });
  });

  describe('chat', () => {
    /** The harnesses as a machine shows them when only the listed ones are installed. */
    function installOnly(store: HarnessStore, ids: string[]): void {
      store.summaries = store.summaries.map((summary) => ({ ...summary, detection: { ...summary.detection, installed: ids.includes(summary.info.id) } }));
    }

    it('says which harnesses can chat', async () => {
      const { store } = await setup();

      expect(store.canChat('omp')).toBe(true);
      expect(store.canChat('opencode')).toBe(false);
    });

    it('keeps the views on the daily driver but sends chats to a harness that can chat', async () => {
      const { store } = await setup({ defaultHarness: 'opencode' });

      expect(store.active?.info.id).toBe('opencode');
      expect(store.chatActive?.info.id).toBe('omp');
      expect(store.chatComingSoon).toBeNull();
    });

    it('chats with the daily driver when it can chat', async () => {
      const { store } = await setup({ defaultHarness: 'pi' });
      await store.pickBinary('pi');

      expect(store.chatActive?.info.id).toBe('pi');
    });

    it('picks the first installed harness that can chat when the daily driver cannot', async () => {
      const { store } = await setup({ defaultHarness: 'opencode' });
      installOnly(store, ['pi', 'opencode']);

      expect(store.chatActive?.info.id).toBe('pi');
    });

    it('reports chat as coming soon when the only installed harnesses cannot chat', async () => {
      const { store } = await setup({ defaultHarness: null });
      installOnly(store, ['opencode']);

      expect(store.chatComingSoon?.info.id).toBe('opencode');
      expect(store.chatActive?.detection.installed).toBe(false);
    });

    it('reports nothing coming soon when no harness is installed at all', async () => {
      const { store } = await setup({ defaultHarness: null });
      installOnly(store, []);

      expect(store.chatComingSoon).toBeNull();
    });
  });
});
