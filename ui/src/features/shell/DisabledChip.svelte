<script lang="ts">
  import type { Snippet } from 'svelte';

  import Tooltip from '$lib/components/Tooltip.svelte';

  interface Props {
    label: string;
    /** Why the control is not usable yet; shown as a tooltip. */
    hint: string;
    content: Snippet;
  }

  let { label, hint, content }: Props = $props();
</script>

<!-- aria-disabled (not disabled) keeps the chip focusable so the hint is reachable by keyboard. -->
<Tooltip text={hint} side="bottom">
  {#snippet children({ describedby })}
    <button
      type="button"
      aria-disabled="true"
      aria-label="{label}. {hint}"
      aria-describedby={describedby}
      class="inline-flex h-8 cursor-not-allowed items-center gap-2 rounded-full border border-line bg-surface-2 px-3 text-sm text-fg-2"
    >
      {@render content()}
    </button>
  {/snippet}
</Tooltip>
