<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';

  import type { ShareSegment } from './share';
  import ShareLegend from './ShareLegend.svelte';
  import { waffleCells } from './waffle';

  interface Props {
    segments: ShareSegment[];
    /** Names the chart for assistive technology: "Failures by cause". */
    label: string;
  }

  let { segments, label }: Props = $props();

  // One square per percent, filled segment after segment in reading order.
  const squares = $derived.by(() => {
    const counts = waffleCells(segments.map((segment) => segment.value));

    return segments.flatMap((segment, index) => Array.from({ length: counts[index] }, () => segment));
  });

  // Whole-pixel squares and gaps that fill the full width: every gap is the same, and the few pixels left over after
  // the closest fit go to single columns spread evenly (a 1 px wider square is invisible; an uneven gap is not).
  let width = $state(0);
  const fit = $derived.by(() => {
    let best = { columns: 20, cell: 4, gap: 3, rest: Infinity };

    for (const columns of [20, 25]) {
      for (let gap = 2; gap <= 4; gap += 1) {
        const cell = Math.floor((width - gap * (columns - 1)) / columns);
        const rest = width - cell * columns - gap * (columns - 1);

        if (cell >= 6 && rest < best.rest) {
          best = { columns, cell, gap, rest };
        }
      }
    }

    const rest = Number.isFinite(best.rest) ? best.rest : 0;
    const tracks = Array.from({ length: best.columns }, (_, column) => {
      const extra = Math.floor(((column + 1) * rest) / best.columns) - Math.floor((column * rest) / best.columns);

      return `${best.cell + extra}px`;
    });

    return { ...best, tracks: tracks.join(' ') };
  });

  const summary = $derived(segments.map((segment) => t('ecosystem.chart.share', { label: segment.label, share: segment.shareLabel })).join(', '));

  // Pointing at a square or a legend row brings that segment forward and quiets the rest.
  let active = $state<string | null>(null);
</script>

<div bind:clientWidth={width} class="flex w-full flex-col gap-4">
  <!-- The legend below carries every exact figure, so the grid itself is one picture for assistive technology. -->
  <div
    role="img"
    aria-label={t('ecosystem.chart.waffle', { label, summary })}
    class="grid"
    style:grid-template-columns={fit.tracks}
    style:grid-auto-rows="{fit.cell}px"
    style:gap="{fit.gap}px"
  >
    {#each squares as segment, cell (cell)}
      <!-- Hovering is a pointer-only extra; the legend gives keyboard and screen-reader users the same figures. -->
      <span
        role="presentation"
        title={segment.description}
        class={cx(
          'rounded-[2px] transition-opacity duration-[var(--dur-fast)]',
          active !== null && active !== segment.key && 'opacity-30',
          motion.level === 'full' && 'waffle-pop',
        )}
        style:background={segment.color}
        style:animation-delay="{cell * 6}ms"
        onpointerenter={() => (active = segment.key)}
        onpointerleave={() => (active = null)}
      ></span>
    {/each}
  </div>

  <ShareLegend {segments} {label} bind:active />
</div>

<style>
  .waffle-pop {
    animation: waffle-pop 320ms var(--ease-out-expo, ease-out) both;
  }

  @keyframes waffle-pop {
    from {
      transform: scale(0.4);
      opacity: 0;
    }
  }
</style>
