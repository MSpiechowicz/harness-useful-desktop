<script lang="ts">
  import { onMount } from 'svelte';

  import Tooltip from '$lib/components/Tooltip.svelte';
  import { motion } from '$lib/stores/motion.svelte';

  import type { BarDatum } from './bars';

  interface Props {
    data: BarDatum[];
    /** Names the group for assistive technology: "Cost by model". */
    label: string;
    /** Bars share this scale; defaults to the largest value, so the top bar fills the track. */
    max?: number;
  }

  let { data, label, max }: Props = $props();

  const scale = $derived(Math.max(max ?? 0, ...data.map((datum) => datum.value), Number.EPSILON));

  // Bars grow from zero once, and only when motion is full (reduced and off keep them still).
  let grown = $state(motion.level !== 'full');

  onMount(() => {
    if (!grown) {
      const frame = requestAnimationFrame(() => (grown = true));

      return () => cancelAnimationFrame(frame);
    }
  });

  /** A nonzero value always leaves a visible sliver. */
  function width(value: number): number {
    return value <= 0 ? 0 : Math.max((value / scale) * 100, 1.5);
  }
</script>

<div role="group" aria-label={label} class="flex flex-col gap-1">
  {#each data as datum (datum.key)}
    <Tooltip text={datum.description} side="top" class="flex w-full">
      {#snippet children({ describedby })}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -- focusable so keyboard users reach the exact values -->
        <div
          role="img"
          tabindex="0"
          aria-label={datum.description}
          aria-describedby={describedby}
          class="grid min-h-8 w-full grid-cols-[minmax(0,7.5rem)_minmax(0,1fr)_auto] items-center gap-3 rounded-sm px-1.5 text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover @lg:grid-cols-[minmax(0,10rem)_minmax(0,1fr)_auto]"
        >
          <span class="truncate text-fg-2" title={datum.label}>{datum.label}</span>

          <svg class="h-2.5 w-full overflow-visible" aria-hidden="true">
            <rect class="fill-surface-3" width="100%" height="100%" rx="5" />
            <rect
              width="{width(datum.value)}%"
              height="100%"
              rx="5"
              style:fill={datum.color}
              style:transform="scaleX({grown ? 1 : 0})"
              class="origin-left transition-[width,transform] duration-[var(--dur-enter)] ease-[var(--ease-out-expo)] [transform-box:fill-box]"
            />
          </svg>

          <span class="tabular min-w-20 text-right text-fg">{datum.valueLabel}</span>
        </div>
      {/snippet}
    </Tooltip>
  {/each}
</div>
