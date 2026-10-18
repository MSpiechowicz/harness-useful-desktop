<script lang="ts">
  import { createRawSnippet } from 'svelte';

  import ViewHeader from '$lib/components/ViewHeader.svelte';

  /** Which view's header is showing; changing it swaps the header the way the shell's view host does (a keyed block). */
  let { which }: { which: string } = $props();

  const menu = createRawSnippet(() => ({ render: () => `<button type="button">Menu</button>` }));
</script>

{#key which}
  <ViewHeader title={which} fixed={menu} />
{/key}
