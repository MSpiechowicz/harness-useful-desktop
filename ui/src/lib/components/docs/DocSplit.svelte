<script lang="ts">
  import type { Snippet } from 'svelte';

  import { escapeCloses } from '$lib/utils/escape-closes';

  interface Props {
    /** The row whose panel is open; null for none. */
    openId: string | null;
    /** Escape closes the open panel from anywhere in the view. */
    onclose: () => void;
    /** The filters and the list. */
    body: Snippet;
    /** The panel of the open row. It slides over the view's right edge, whatever the width: the filters and the list keep all of it. */
    panel: Snippet;
  }

  let { openId, onclose, body, panel }: Props = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="absolute inset-0 flex overflow-clip" onkeydown={(event) => escapeCloses(event, openId !== null, onclose)}>
  <div class="flex min-h-0 min-w-0 flex-1 flex-col">
    {@render body()}
  </div>

  {@render panel()}
</div>
