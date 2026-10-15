<script lang="ts">
  import type { Snippet } from 'svelte';

  import { scrollMemory } from '$lib/a11y/scroll-memory';

  import ListDrawer from './ListDrawer.svelte';
  import type { PaneDrawer } from './pane-drawer.svelte';
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
    /** On narrow windows the list moves into this drawer (opened by a `DrawerToggle`) and the detail takes the whole width. */
    drawer?: PaneDrawer;
    /** Accessible name of the drawer; required with `drawer`. */
    drawerLabel?: string;
    list: Snippet;
    detail: Snippet;
  }

  let { label, size = $bindable(320), scrollKey, min = 240, minDetail = 320, drawer, drawerLabel = '', list, detail }: Props = $props();
</script>

{#snippet listPane()}
  <div class="relative flex size-full flex-col overflow-hidden">{@render list()}</div>
{/snippet}

{#snippet detailPane()}
  {#if scrollKey}
    <div class="@container relative size-full overflow-y-auto" use:scrollMemory={scrollKey}>{@render detail()}</div>
  {:else}
    <div class="@container relative size-full overflow-y-auto">{@render detail()}</div>
  {/if}
{/snippet}

{#snippet columns()}
  <SplitPane {label} bind:size {min} minSecond={minDetail}>
    {#snippet first()}{@render listPane()}{/snippet}
    {#snippet second()}{@render detailPane()}{/snippet}
  </SplitPane>
{/snippet}

<!-- A resizable master/detail layout; each pane scrolls on its own, and the detail pane is a container for responsive content. -->
{#if drawer}
  <div bind:clientWidth={drawer.width} class="relative size-full min-h-0">
    {#if drawer.active}
      {@render detailPane()}

      <ListDrawer {drawer} label={drawerLabel}>{@render listPane()}</ListDrawer>
    {:else}
      {@render columns()}
    {/if}
  </div>
{:else}
  {@render columns()}
{/if}
