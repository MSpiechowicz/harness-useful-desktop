<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import type { ShareSegment } from './share';

  interface Props {
    segments: ShareSegment[];
    label: string;
    /** The segment under the pointer, shared with the chart beside the legend. */
    active?: string | null;
  }

  let { segments, label, active = $bindable(null) }: Props = $props();
</script>

<!-- The legend is the table view: every part named with its exact figure and share, so color never stands alone. -->
<ul class="tabular flex min-w-0 flex-col text-sm" aria-label={t('ecosystem.chart.values', { label })}>
  {#each segments as segment (segment.key)}
    <li
      class={cx(
        '-mx-1.5 grid grid-cols-[auto_minmax(0,1fr)_auto_3rem] items-baseline gap-x-3 rounded-sm px-1.5 py-1.5 transition-colors duration-[var(--dur-fast)]',
        active === segment.key && 'bg-hover',
      )}
      onpointerenter={() => (active = segment.key)}
      onpointerleave={() => (active = null)}
    >
      <span class="size-2.5 self-center rounded-[3px]" style:background={segment.color} aria-hidden="true"></span>
      <span class="flex min-w-0 flex-col">
        <span class="truncate text-fg-2" title={segment.label}>{segment.label}</span>
        {#if segment.note}<span class="truncate text-xs text-fg-3" title={segment.note}>{segment.note}</span>{/if}
      </span>
      <span class="text-right text-fg">{segment.valueLabel}</span>
      <span class="text-right text-fg-3">{segment.shareLabel}</span>
    </li>
  {/each}
</ul>
