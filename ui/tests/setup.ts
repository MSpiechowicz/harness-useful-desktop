import '@testing-library/jest-dom/vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { afterEach } from 'vitest';

import { samplePrefs } from './helpers';

// jsdom (used by the few tests that need a real HTML sanitizer) has no matchMedia.
if (typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string) =>
    ({ matches: false, media: query, addEventListener: () => {}, removeEventListener: () => {}, addListener: () => {}, removeListener: () => {} }) as unknown as MediaQueryList;
}

// happy-dom rejects `Animation.finished` on cancel() without marking it handled (the Web Animations spec does),
// so every cancelled Svelte transition would surface as an unhandled AbortError. jsdom has no `Animation` at all.
if (typeof Animation !== 'undefined') {
  const cancel = Animation.prototype.cancel;

  Animation.prototype.cancel = function (this: Animation): void {
    this.finished.catch(() => {});
    cancel.call(this);
  };
}

// happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
prefs.value = samplePrefs({ motion: 'off' });

// Views remember what was selected; a test must not inherit the previous one's selection.
afterEach(() => viewState.resetAll());

// A test that switched language must not leave the next one reading it. English applies synchronously.
afterEach(() => void i18n.load('en'));
