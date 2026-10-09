import '@testing-library/jest-dom/vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from './helpers';

// jsdom (used by the few tests that need a real HTML sanitizer) has no matchMedia.
if (typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string) =>
    ({ matches: false, media: query, addEventListener: () => {}, removeEventListener: () => {}, addListener: () => {}, removeListener: () => {} }) as unknown as MediaQueryList;
}

// happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
prefs.value = samplePrefs({ motion: 'off' });
