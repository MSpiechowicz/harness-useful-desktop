<script lang="ts">
  import type { Snippet } from 'svelte';

  import { scrollMemory } from '$lib/a11y/scroll-memory';

  import SplitPane from './SplitPane.svelte';

  interface Props {
    /** Accessible name of the draggable divider. */
    label: string;
    size?: number;
    /** Remembers the detail pane's scroll position under this key while the user is in other views. */
    scrollKey?: string;
    /** Smallest width of the list pane; the detail pane keeps at least `minDetail`. */
    min?: number;
    minDetail?: number;
    list: Snippet;
    detail: Snippet;
  }

  let { label, size = $bindable(320), scrollKey, min = 240, minDetail = 320, list, detail }: Props = $props();
</script>

<!-- A resizable master/detail layout; each pane scrolls on its own, and the detail pane is a container for responsive content. -->
<SplitPane {label} bind:size {min} minSecond={minDetail}>
  {#snippet first()}
    <div class="relative flex size-full flex-col overflow-hidden">{@render list()}</div>
  {/snippet}
  {#snippet second()}
    {#if scrollKey}
      <div class="@container relative size-full overflow-y-auto" use:scrollMemory={scrollKey}>{@render detail()}</div>
    {:else}
      <div class="@container relative size-full overflow-y-auto">{@render detail()}</div>
    {/if}
  {/snippet}
</SplitPane>
