import { describe, expect, it } from 'vitest';

import { fakeTransport, samplePrefs } from '../../../tests/helpers';
import { createMockTransport } from '../ipc/mock/transport';
import { defaultScenario } from '../ipc/mock/scenarios';

import { HarnessStore } from './harness.svelte';
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

  return { store, prefs, toasts };
}

describe('HarnessStore', () => {
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

  it('re-detects every harness on rescan', async () => {
    const { store } = await setup();
    const before = store.get('omp');

    await store.rescan();

    expect(store.scanning).toBe(false);
    expect(store.summaries).toHaveLength(2);
    expect(store.get('omp')).toEqual(before);
  });
});
