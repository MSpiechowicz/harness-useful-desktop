import { OTHER, makerModelHex, modelFamily, poolHex, providerHex } from '$lib/dashboard/palette';
import { t } from '$lib/i18n/i18n.svelte';
import { theme } from '$lib/stores/theme.svelte';

/** The same harness always gets the same color, the one the Harness Dashboard gives it; it follows the theme. */
export function providerColor(key: string): string {
  return providerHex(key, theme.resolved);
}

/** Bars of a ranking that has no category meaning of its own share the accent. */
export const ACCENT_COLOR = 'var(--accent-2)';

export type MeterTone = 'accent' | 'warning' | 'danger';

/** Where a usage meter turns from calm to warning to danger. */
export function meterTone(percent: number): MeterTone {
  if (percent >= 90) {
    return 'danger';
  }

  return percent >= 75 ? 'warning' : 'accent';
}

/** The words that say what the color says, so color is never the only signal. */
export const TONE_WORDS: Record<MeterTone, string | null> = {
  accent: null,
  get warning() {
    return t('ecosystem.meter.high');
  },
  get danger() {
    return t('ecosystem.meter.full');
  },
};

export const TONE_FILLS: Record<MeterTone, string> = {
  accent: 'bg-accent',
  warning: 'bg-warning',
  danger: 'bg-danger',
};

/**
 * A model's color, the way the Harness Dashboard paints it. `peers` are all the models on screen, largest first
 * (the desktop has no all-time ranking, so the current cost order stands in for the dashboard's): models of
 * Anthropic and OpenAI take their provider's shades ranked within the maker, the top one wearing the provider's own
 * color; other makers' models take the general pool in order, past its end the "Other" gray.
 */
export function modelColor(model: string, peers: readonly string[]): string {
  const mode = theme.resolved;
  const shaded = makerModelHex(model, peers, mode);

  if (shaded) {
    return shaded;
  }

  return poolHex(peers.filter((peer) => !modelFamily(peer)).indexOf(model), mode);
}

/** The `index`-th value of a dimension that owns no family wears the dashboard's general pool, in order. */
export function poolColor(index: number): string {
  return poolHex(index, theme.resolved);
}

/** Everything folded into "Other". */
export function otherColor(): string {
  return OTHER[theme.resolved];
}
