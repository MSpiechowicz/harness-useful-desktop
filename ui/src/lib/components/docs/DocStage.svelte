<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<!-- Everything that is bigger than the view is clipped here, not just hidden: a clipped box cannot be scrolled at all, so
     nothing in the view (a focused row, a panel on its way in) can push the shell around it up or down. -->
<div class="relative min-h-0 flex-1 overflow-clip">
  {@render children()}
</div>
