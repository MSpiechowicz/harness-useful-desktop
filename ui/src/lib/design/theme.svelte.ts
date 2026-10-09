import type { ThemePref } from '$lib/ipc/bindings/ThemePref';

export type ResolvedTheme = 'dark' | 'light';

/** Reactive wrapper around a CSS media query (`prefers-color-scheme`, `prefers-reduced-motion`, ...). */
export class MediaQuery {
  matches = $state(false);

  private readonly list: MediaQueryList | null;
  private readonly onChange = (event: MediaQueryListEvent): void => {
    this.matches = event.matches;
  };

  constructor(query: string, source: Pick<Window, 'matchMedia'> = window) {
    this.list = typeof source.matchMedia === 'function' ? source.matchMedia(query) : null;
    this.matches = this.list?.matches ?? false;
    this.list?.addEventListener('change', this.onChange);
  }

  destroy(): void {
    this.list?.removeEventListener('change', this.onChange);
  }
}

/** `system` follows the OS; explicit choices win. */
export function resolveTheme(preference: ThemePref, systemPrefersDark: boolean): ResolvedTheme {
  if (preference === 'system') {
    return systemPrefersDark ? 'dark' : 'light';
  }

  return preference;
}
