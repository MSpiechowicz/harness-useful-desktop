<script lang="ts">
  import type { Snippet } from 'svelte';

  import { rise } from '$lib/design/motion';
  import { WIDE_FROM_PX } from '$lib/design/layout';
  import { escapeCloses } from '$lib/utils/escape-closes';

  import DetailEmpty from '../DetailEmpty.svelte';
  import type { IconComponent } from '../icon';
  import { SIDE_PANEL_WIDTH } from '../side-panel';

  interface Props {
    /** The row whose panel is open; null for none. */
    openId: string | null;
    /** The icon and the line of the empty detail column of a wide view. */
    emptyIcon: IconComponent;
    emptyMessage: string;
    /** Escape closes the open panel from anywhere in the view. */
    onclose: () => void;
    /** The filters and the list. */
    body: Snippet;
    /** The panel of the open row, and whether it fills a column (a wide view) or slides over the view's edge. */
    panel: Snippet<[boolean]>;
  }

  let { openId, emptyIcon, emptyMessage, onclose, body, panel }: Props = $props();

  let width = $state(0);

  const wide = $derived(width >= WIDE_FROM_PX);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div bind:clientWidth={width} class="absolute inset-0 flex overflow-clip" onkeydown={(event) => escapeCloses(event, openId !== null, onclose)}>
  <!-- Below the wide layout the panel slides over the list: the filters and the list keep their width. -->
  <div class="flex min-h-0 min-w-0 flex-1 flex-col">
    {@render body()}
  </div>

  {#if wide}
    <!-- The detail has its own column, always there: what is chosen swaps in with a short fade, with nothing sliding. -->
    <div class="relative flex shrink-0 flex-col border-l border-panel-edge bg-surface-1" style:width={SIDE_PANEL_WIDTH}>
      {#if openId === null}
        <DetailEmpty icon={emptyIcon} message={emptyMessage} />
      {:else}
        {#key openId}
          <div in:rise={{ y: 6, duration: 240 }} class="flex min-h-0 flex-1 flex-col">
            {@render panel(true)}
          </div>
        {/key}
      {/if}
    </div>
  {:else}
    {@render panel(false)}
  {/if}
</div>
