import { render, screen } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { updates } from '$lib/stores/updates.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { intro } from '../intro/intro.svelte';
import { onboarding } from '../onboarding/onboarding.svelte';
import Shell from '../shell/Shell.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: false });
  await harness.load();
});

describe('update check at startup', () => {
  it('waits for the intro and for setup, then shows the notice for an available update', async () => {
    intro.phase = 'playing';
    render(Shell);
    await new Promise((resolve) => setTimeout(resolve, 60));
    expect(updates.state.kind).toBe('idle');

    intro.phase = 'done';
    expect(onboarding.visible).toBe(true);
    await new Promise((resolve) => setTimeout(resolve, 60));
    expect(updates.state.kind).toBe('idle');

    prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: true });

    expect(await screen.findByRole('region', { name: 'Update' })).toHaveTextContent('0.2.0');
    expect(updates.state.kind).toBe('available');
  });
});
