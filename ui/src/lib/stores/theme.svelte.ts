import { MediaQuery, resolveTheme, type ResolvedTheme } from '$lib/design/theme.svelte';
import type { ThemePref } from '$lib/ipc/bindings/ThemePref';

import { prefs } from './prefs.svelte';

export class ThemeStore {
  private readonly systemDark: MediaQuery;

  constructor(
    private readonly preference: () => ThemePref,
    systemDark: MediaQuery = new MediaQuery('(prefers-color-scheme: dark)'),
    private readonly root: HTMLElement = document.documentElement,
  ) {
    this.systemDark = systemDark;
  }

  /** The theme actually applied: the preference with `system` resolved against the OS. */
  get resolved(): ResolvedTheme {
    return resolveTheme(this.preference(), this.systemDark.matches);
  }

  /** Mirrors the resolved theme onto `<html data-theme>`; returns a disposer. */
  start(): () => void {
    return $effect.root(() => {
      $effect(() => {
        this.root.dataset.theme = this.resolved;
      });
    });
  }
}

export const theme = new ThemeStore(() => prefs.value?.theme ?? 'system');
