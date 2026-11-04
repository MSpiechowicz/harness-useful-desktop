<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** Fills the old (left) half; omit it to leave that half empty. */
    left?: Snippet;
    /** Fills the new (right) half; omit it to leave that half empty. */
    right?: Snippet;
  }

  let { left, right }: Props = $props();
</script>

<!-- What goes under a split row (a comment thread or the composer) sits in the half of the side it is about, level with that
     half's code (past the line number and the sign), and leaves the other half empty at that height. -->
<div class="grid grid-cols-2 divide-x divide-line">
  {#each [{ side: 'old', content: left }, { side: 'new', content: right }] as const as half (half.side)}
    <div class="min-w-0" data-half={half.side}>
      {#if half.content}
        <div class="py-2 pr-3 pl-18">{@render half.content()}</div>
      {/if}
    </div>
  {/each}
</div>
