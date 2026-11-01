<script lang="ts">
  import type { Snippet } from 'svelte';

  import DocStage from '$lib/components/docs/DocStage.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import FilterRow from '$lib/components/FilterRow.svelte';
  import type { IconComponent } from '$lib/components/icon';

  import KnowledgeTabs from './KnowledgeTabs.svelte';

  interface Props {
    icon: IconComponent;
    title: string;
    body: string;
    action?: Snippet;
  }

  let { icon, title, body, action }: Props = $props();
</script>

<!-- A list with nothing in it is still one of two: the switch stays, so the other list can be reached, and the filters have nothing to narrow. -->
<DocStage>
  <div class="absolute inset-0 flex flex-col">
    <FilterRow><KnowledgeTabs /></FilterRow>

    <div class="grid min-h-0 flex-1 place-items-center overflow-y-auto">
      <EmptyState {icon} {title} {body} {action} />
    </div>
  </div>
</DocStage>
