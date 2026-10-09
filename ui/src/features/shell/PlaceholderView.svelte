<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';

  import type { ViewMeta } from './views';

  interface Props {
    view: ViewMeta;
  }

  let { view }: Props = $props();
</script>

<div class="grid size-full place-items-center">
  <EmptyState icon={view.icon} title={view.heading} body={view.body}>
    {#snippet action()}
      <Badge tone="accent" dot>Coming soon</Badge>
    {/snippet}
  </EmptyState>
</div>
