import { flushSync } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { language } from './language.svelte';
import { prefs } from './prefs.svelte';

let stop = (): void => {};

afterEach(() => {
  stop();
  prefs.value = samplePrefs({ motion: 'off' });
});

/** The saved language, as the preferences store holds it. */
const preference = {
  set code(code: string | undefined) {
    prefs.value = code === undefined ? null : samplePrefs({ motion: 'off', language: code });
  },
};

/** Gives the language store the time it would need to switch, to show that it did not. */
async function settled(): Promise<void> {
  flushSync();
  await new Promise((resolve) => setTimeout(resolve, 50));
}

describe('LanguageStore', () => {
  it('follows the saved language, a change at a time', async () => {
    stop = language.start();

    preference.code = 'pl';
    flushSync();
    await vi.waitFor(() => expect(i18n.lang).toBe('pl'));

    preference.code = 'de';
    flushSync();
    await vi.waitFor(() => expect(i18n.lang).toBe('de'));
  });

  it('goes back to English for a language it does not know, and when the preference is gone', async () => {
    stop = language.start();

    preference.code = 'fr';
    flushSync();
    await vi.waitFor(() => expect(i18n.lang).toBe('fr'));

    preference.code = 'xx';
    flushSync();
    await vi.waitFor(() => expect(i18n.lang).toBe('en'));

    preference.code = 'es';
    flushSync();
    await vi.waitFor(() => expect(i18n.lang).toBe('es'));
    preference.code = undefined;
    flushSync();
    await vi.waitFor(() => expect(i18n.lang).toBe('en'));
  });

  it('stops following once disposed', async () => {
    stop = language.start();
    stop();

    preference.code = 'pl';
    await settled();

    expect(i18n.lang).toBe('en');
  });
});
