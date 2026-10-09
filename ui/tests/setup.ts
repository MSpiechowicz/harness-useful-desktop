import '@testing-library/jest-dom/vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from './helpers';

// happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
prefs.value = samplePrefs({ motion: 'off' });
