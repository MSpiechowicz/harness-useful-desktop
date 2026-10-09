<script lang="ts">
  import type { Snippet } from 'svelte';

  import Tooltip from '$lib/components/Tooltip.svelte';

  interface Props {
    /** Spoken and shown on hover: what the item is and where it leads. */
    label: string;
    shortcut?: string;
    onclick: () => void;
    content: Snippet;
  }

  let { label, shortcut, onclick, content }: Props = $props();
</script>

<Tooltip text={label} {shortcut} side="top">
  {#snippet children({ describedby })}
    <button type="button" aria-label={label} aria-describedby={describedby} {onclick} class="status-item">
      {@render content()}
    </button>
  {/snippet}
</Tooltip>
