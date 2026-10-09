<script lang="ts">
  import type { Snippet } from 'svelte';

  import SplitPane from './SplitPane.svelte';

  interface Props {
    /** Accessible name of the draggable divider. */
    label: string;
    size?: number;
    list: Snippet;
    detail: Snippet;
  }

  let { label, size = $bindable(320), list, detail }: Props = $props();
</script>

<!-- A resizable master/detail layout; each pane scrolls on its own. -->
<SplitPane {label} bind:size min={240} minSecond={320}>
  {#snippet first()}
    <div class="flex size-full flex-col overflow-hidden">{@render list()}</div>
  {/snippet}
  {#snippet second()}
    <div class="size-full overflow-y-auto">{@render detail()}</div>
  {/snippet}
</SplitPane>
