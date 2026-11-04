<script lang="ts">
  import HighlightedText from '$lib/components/HighlightedText.svelte';
  import { matchIndices } from '$lib/components/config-tree';
  import { cx } from '$lib/utils/cx';

  import { meterBars, METER_SIZE } from './structured';

  interface Props {
    level: string;
    /** Normalized search needle; empty highlights nothing. */
    needle?: string;
  }

  let { level, needle = '' }: Props = $props();

  const BAR_HEIGHTS = ['h-[4px]', 'h-[7px]', 'h-[10px]', 'h-[12px]'];
</script>

<span class="inline-flex items-center gap-2 text-xs text-fg-2">
  <span class="inline-flex h-[12px] items-end gap-[2px]" aria-hidden="true">
    {#each { length: METER_SIZE } as _, bar (bar)}
      <i class={cx('w-[3px] rounded-[1px]', BAR_HEIGHTS[bar], bar < meterBars(level) ? 'bg-accent' : 'bg-surface-4')}></i>
    {/each}
  </span>
  <HighlightedText text={level} indices={matchIndices(level, needle)} />
</span>
