<script lang="ts" module>
  /** Below this share of `total` the meter asks for attention: the fill turns from the accent to the warning tone. */
  export const LOW_USAGE_RATIO = 0.2;

  /** Whether `value` out of `total` is low enough to warn about. */
  export function lowUsage(value: number, total: number): boolean {
    return total > 0 && value / total < LOW_USAGE_RATIO;
  }
</script>

<script lang="ts">
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** How many of `total` count, such as the times a note was found useful. */
    value: number;
    /** How many there were, such as the times the note was shown. Above zero: a meter of nothing is not drawn. */
    total: number;
    /** What the meter measures, for people who cannot see it. */
    label: string;
  }

  let { value, total, label }: Props = $props();

  const share = $derived(total > 0 ? Math.min(1, Math.max(0, value / total)) : 0);
  const low = $derived(lowUsage(value, total));
</script>

<!-- Styled like the countdown of a script request: a thin track with a rounded fill. -->
<div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={total} aria-valuenow={value} class="h-1 w-24 shrink-0 overflow-hidden rounded-full bg-surface-4">
  <div class={cx('h-full rounded-full transition-[width,background-color] duration-300 ease-linear', low ? 'bg-warning' : 'bg-accent')} style:width="{share * 100}%"></div>
</div>
