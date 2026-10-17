<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** How much there is in the list ("3 hooks"), said once, before the filters. */
    count: string;
    /** The filter menus, left to right. */
    children: Snippet;
    /** Something quiet at the far end of the row, e.g. a hint. */
    trailing?: Snippet;
  }

  let { count, children, trailing }: Props = $props();
</script>

<!-- The row above a list: how much there is, then the filters that narrow it. The same in every view that has one. -->
<div class="flex h-14 shrink-0 items-center gap-3 border-b border-line px-4">
  <p class="tabular shrink-0 text-sm font-medium text-fg-2">{count}</p>

  <div class="flex min-w-0 items-center gap-2">
    {@render children()}
  </div>

  {#if trailing}
    <div class="ml-auto flex min-w-0 items-center gap-2">
      {@render trailing()}
    </div>
  {/if}
</div>
