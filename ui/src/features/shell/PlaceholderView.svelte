<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';

  import type { ViewMeta } from './views';

  interface Props {
    view: ViewMeta;
  }

  let { view }: Props = $props();
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title={view.label} icon={view.icon} />

  <div class="grid min-h-0 flex-1 place-items-center overflow-y-auto">
    <EmptyState icon={view.icon} title={view.heading} body={view.body}>
      {#snippet action()}
        <Badge tone="accent" dot>Coming soon</Badge>
      {/snippet}
    </EmptyState>
  </div>
</div>
