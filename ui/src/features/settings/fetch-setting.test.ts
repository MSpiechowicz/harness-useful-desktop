import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off' });
  Element.prototype.scrollIntoView = vi.fn();
});

describe('Settings, Projects: fetching the open project', () => {
  it('is on by default, and the user can switch it off', async () => {
    render(SettingsView);
    const toggle = screen.getByRole('switch', { name: 'Fetch the open project in the background' });

    expect(toggle).toBeChecked();

    await fireEvent.click(toggle);

    await waitFor(() => expect(prefs.value?.backgroundFetch).toBe(false));
    expect(toggle).not.toBeChecked();
  });
});
