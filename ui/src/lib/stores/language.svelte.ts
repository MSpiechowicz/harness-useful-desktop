import { untrack } from 'svelte';

import { i18n, toLang } from '$lib/i18n/i18n.svelte';

import { prefs } from './prefs.svelte';

/** Keeps the language on screen equal to the saved one: a pick, a rollback and a late first load all switch it. */
export class LanguageStore {
  constructor(private readonly preference: () => string | undefined) {}

  /** Follows the saved language for the app's lifetime; returns a disposer. */
  start(): () => void {
    return $effect.root(() => {
      $effect(() => {
        const code = this.preference();

        untrack(() => void i18n.load(toLang(code)));
      });
    });
  }
}

export const language = new LanguageStore(() => prefs.value?.language);
