<script lang="ts">
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';

  import type { ShareSegment } from './share';
  import ShareLegend from './ShareLegend.svelte';

  interface Props {
    segments: ShareSegment[];
    /** Names the chart for assistive technology: "Cost by model". */
    label: string;
    /** The figure in the hole, e.g. the total cost, and the word under it. */
    total: string;
    caption: string;
  }

  let { segments, label, total, caption }: Props = $props();

  // A fixed 160 px ring drawn 1:1, so its edges land on whole pixels; arcs keep a small gap so neighbours never blend.
  const SIZE = 160;
  const RADIUS = 66;
  const STROKE = 16;
  const GAP_DEGREES = 1.6;

  const sum = $derived(segments.reduce((acc, segment) => acc + segment.value, 0));

  function point(degrees: number): string {
    const angle = ((degrees - 90) * Math.PI) / 180;

    return `${SIZE / 2 + RADIUS * Math.cos(angle)} ${SIZE / 2 + RADIUS * Math.sin(angle)}`;
  }

  const arcs = $derived.by(() => {
    const gap = segments.length > 1 ? GAP_DEGREES : 0;
    let start = 0;

    return segments.map((segment) => {
      const sweep = sum > 0 ? (segment.value / sum) * 360 : 0;
      const from = start + gap / 2;
      // A lone segment closes the ring just short of a full circle, which an arc command cannot draw.
      const to = Math.max(from + 0.01, Math.min(start + sweep - gap / 2, from + 359.99));
      start += sweep;

      return { segment, d: `M ${point(from)} A ${RADIUS} ${RADIUS} 0 ${to - from > 180 ? 1 : 0} 1 ${point(to)}` };
    });
  });

  const summary = $derived(segments.map((segment) => `${segment.label} ${segment.valueLabel} (${segment.shareLabel})`).join(', '));

  // Pointing at an arc or a legend row brings that part forward and quiets the rest.
  let active = $state<string | null>(null);
</script>

<!-- Tall enough for the most the legend can show (five parts and a two-line "other"), so switching what the ring
     groups by never resizes the card. -->
<div class="flex flex-col items-center gap-6 @xl:min-h-[13rem] @xl:flex-row @xl:items-center">
  <div role="img" aria-label="{label}: {total} {caption}; {summary}" class="relative shrink-0" style:width="{SIZE}px" style:height="{SIZE}px">
    <svg width={SIZE} height={SIZE} viewBox="0 0 {SIZE} {SIZE}" class={cx(motion.level === 'full' && 'donut-spin')} aria-hidden="true">
      <circle cx={SIZE / 2} cy={SIZE / 2} r={RADIUS} fill="none" stroke="var(--surface-3)" stroke-width={STROKE} />
      {#each arcs as { segment, d } (segment.key)}
        <path
          role="presentation"
          {d}
          fill="none"
          stroke={segment.color}
          stroke-width={STROKE}
          class={cx('transition-opacity duration-[var(--dur-fast)]', active !== null && active !== segment.key && 'opacity-30')}
          onpointerenter={() => (active = segment.key)}
          onpointerleave={() => (active = null)}
        >
          <title>{segment.description}</title>
        </path>
      {/each}
    </svg>

    <div class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center" aria-hidden="true">
      <span class="tabular text-lg leading-6 font-semibold text-fg">{total}</span>
      <span class="text-xs text-fg-3">{caption}</span>
    </div>
  </div>

  <div class="w-full min-w-0 flex-1">
    <ShareLegend {segments} {label} bind:active />
  </div>
</div>

<style>
  /* The ring sweeps in once from 12 o'clock. */
  .donut-spin {
    animation: donut-in 700ms var(--ease-out-expo, ease-out) both;
  }

  @keyframes donut-in {
    from {
      opacity: 0;
      transform: rotate(-90deg) scale(0.92);
    }
  }
</style>
