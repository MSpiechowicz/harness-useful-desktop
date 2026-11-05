<script lang="ts">
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<!-- The text under an open row: no box of its own, a thin line from the row's chevron ties it to the row, and the text starts where the row's title does. -->
<div class="mt-0.5 mb-2.5 ml-[var(--outline-rail-x)] flex flex-col gap-2.5 border-l border-line pr-2 pl-[var(--outline-rail-pad)]">
  {@render children()}
</div>
