<script lang="ts">
  import type { Dial, LimitMode } from '$lib/dashboard/limit-dial';

  import { barBlocks } from './limit-gauge';

  interface Props {
    /** The provider's color: the blocks are never recolored for a warning. */
    color: string;
    dial: Dial;
    mode: LimitMode;
    /** Thinner, when four or more limits share the card. */
    small: boolean;
  }

  let { color, dial, mode, small }: Props = $props();

  // Twenty blocks, lit from the 0% end like the segmented arc, pale in the forecast stretch and red in the red end.
  const blocks = $derived(barBlocks(dial, mode, color));
</script>

<!-- A picture: the limit's meter around it carries the words. -->
<div class="flex gap-[var(--limit-block-gap)]" style:height={small ? 'var(--limit-bar-h-small)' : 'var(--limit-bar-h)'} aria-hidden="true">
  {#each blocks as block, i (i)}
    <b class="min-w-0 flex-1 rounded-[var(--limit-block-radius)]" style:background={block.fill} style:opacity={block.opacity}></b>
  {/each}
</div>
