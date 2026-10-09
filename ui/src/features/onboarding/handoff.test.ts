import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { intro } from '../intro/intro.svelte';
import { onboarding } from './onboarding.svelte';

beforeEach(() => {
  prefs.value = samplePrefs({ onboardingCompleted: false });
});

afterEach(() => intro.skip());

describe('first-run setup and the intro', () => {
  it('waits while the logo plays, then opens as the intro hands over', () => {
    intro.play('full');
    expect(onboarding.visible).toBe(false);

    intro.launched();
    expect(intro.phase).toBe('handoff');
    expect(onboarding.visible).toBe(true);
  });

  it('opens at once when the intro is skipped or off', () => {
    intro.play('off');

    expect(onboarding.visible).toBe(true);
  });

  it('opens with the quick crossfade under reduced motion', () => {
    intro.play('reduced');

    expect(onboarding.visible).toBe(true);
  });

  it('stays closed for someone who has finished setup', () => {
    prefs.value = samplePrefs({ onboardingCompleted: true });
    intro.play('reduced');

    expect(onboarding.visible).toBe(false);
  });
});
