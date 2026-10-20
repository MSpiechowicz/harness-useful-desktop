<script lang="ts">
  import type { ComponentProps } from 'svelte';

  import ScopeLabel from '../ScopeLabel.svelte';

  let props: ComponentProps<typeof ScopeLabel> = $props();
</script>

<!-- The Scope column of a documents table. -->
<td role="gridcell" class="border-b border-line/60 px-3 text-fg-2">
  <ScopeLabel {...props} />
</td>
