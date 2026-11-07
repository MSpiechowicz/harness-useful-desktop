<script lang="ts">
  import type { LimitChip } from '$lib/dashboard/limit-dial';
  import { cx } from '$lib/utils/cx';

  interface Props {
    chip: LimitChip;
    /** Smaller, when four or more gauges share a row: neighbours never touch. */
    dense?: boolean;
  }

  let { chip, dense = false }: Props = $props();

  // One style for every state, a solid fill without a border, only the color changes. White text sits on the two fills
  // (green, red), like on the primary and danger buttons.
  const TONES: Record<LimitChip['kind'], string> = {
    ok: 'bg-success-fill text-on-accent',
    out: 'bg-danger-fill text-on-accent',
    idle: 'bg-surface-4 text-fg-2',
  };
</script>

<!-- One width for every chip, the longest text in any language ("Épuisé mer. 04:37"), so a row of them reads as one set.
     The text is centred. A narrower column still wins (max-w-full), and a text too long ends in "…". -->
<span
  class={cx(
    'inline-flex max-w-full items-center justify-center rounded-full leading-[var(--caption-line)] font-semibold whitespace-nowrap',
    dense ? 'w-[var(--limit-chip-w-dense)] px-1 py-px text-2xs' : 'w-[var(--limit-chip-w)] px-2 py-0.5 text-xs',
    TONES[chip.kind],
  )}
  title={chip.text}
>
  <span class="min-w-0 truncate">{chip.text}</span>
</span>
