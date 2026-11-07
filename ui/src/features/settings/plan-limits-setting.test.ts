import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(async () => {
  // A save still on its way would put its answer over the next test's preferences.
  await new Promise((resolve) => setTimeout(resolve, 10));
  vi.restoreAllMocks();
});

/** What `prefs_set` was last asked to save. */
function lastSaved(spy: { mock: { calls: unknown[][] } }) {
  const saves = spy.mock.calls.filter((call) => call[0] === 'prefs_set');

  return (saves.at(-1)?.[1] as { prefs: NonNullable<typeof prefs.value> }).prefs;
}

describe('Settings, Appearance: plan limits', () => {
  it('counts what is used and draws the forecast by default', () => {
    prefs.value = samplePrefs({ motion: 'off' });
    render(SettingsView);

    const group = screen.getByRole('radiogroup', { name: 'Plan limits show' });

    expect(within(group).getByRole('radio', { name: 'Used' })).toBeChecked();
    expect(screen.getByRole('switch', { name: 'Show the forecast' })).toBeChecked();
  });

  it('saves a switch to counting what is left through prefs_set', async () => {
    prefs.value = samplePrefs({ motion: 'off' });
    const spy = vi.spyOn(ipc, 'invoke');
    render(SettingsView);

    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Plan limits show' })).getByRole('radio', { name: 'Left' }));

    await waitFor(() => expect(prefs.value?.limitBars).toBe('left'));
    expect(lastSaved(spy).limitBars).toBe('left');
    expect(within(screen.getByRole('radiogroup', { name: 'Plan limits show' })).getByRole('radio', { name: 'Left' })).toBeChecked();
  });

  it('saves the forecast switch through prefs_set', async () => {
    prefs.value = samplePrefs({ motion: 'off' });
    const spy = vi.spyOn(ipc, 'invoke');
    render(SettingsView);

    await fireEvent.click(screen.getByRole('switch', { name: 'Show the forecast' }));

    await waitFor(() => expect(prefs.value?.limitForecast).toBe(false));
    expect(lastSaved(spy).limitForecast).toBe(false);
    expect(screen.getByRole('switch', { name: 'Show the forecast' })).not.toBeChecked();
  });

  it('is found by searching for plan limits', async () => {
    prefs.value = samplePrefs({ motion: 'off' });
    render(SettingsView);

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search settings' }), { target: { value: 'forecast' } });

    expect(await screen.findByRole('switch', { name: 'Show the forecast' })).toBeInTheDocument();
  });
});
