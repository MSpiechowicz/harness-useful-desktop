/** Category colors come from theme tokens, so every chart follows dark and light. */
const PROVIDER_COLORS: Record<string, string> = {
  omp: 'var(--harness-omp)',
  pi: 'var(--harness-pi)',
  claude: 'var(--accent-1)',
  codex: 'var(--info)',
};

const FALLBACK_COLOR = 'var(--text-3)';

/** The same harness always gets the same color; it matches the OMP and PI badges. */
export function providerColor(key: string): string {
  return PROVIDER_COLORS[key.toLowerCase()] ?? FALLBACK_COLOR;
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
export const TONE_WORDS: Record<MeterTone, string | null> = { accent: null, warning: 'High', danger: 'Nearly full' };

export const TONE_FILLS: Record<MeterTone, string> = {
  accent: 'bg-accent',
  warning: 'bg-warning',
  danger: 'bg-danger',
};
