<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<!-- The small caps label above a block of the branch menu. The line is as tall as the text, so the gap under it is the gap you see. -->
<div class="mb-1.5 px-2.5 pt-1 text-xs leading-4 font-medium tracking-wide text-fg-3 uppercase">{@render children()}</div>
