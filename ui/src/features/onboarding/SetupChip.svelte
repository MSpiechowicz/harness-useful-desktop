<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import Spinner from '$lib/components/Spinner.svelte';

  import type { Chip } from './setup-model';

  interface Props {
    chip: Chip;
  }

  let { chip }: Props = $props();
</script>

{#if chip.none}
  <!-- Nothing to set up where there is no harness: a quiet dash, not a state. -->
  <span role="img" aria-label={chip.label} class="px-3 text-sm text-fg-3">—</span>
{:else}
  <!-- The wizard's one badge size; the tone says the state, and a spinner shows while the item is being set up. -->
  <Badge size="lg" tone={chip.tone}>
    {#if chip.busy}
      <Spinner size={12} />
    {/if}
    {chip.label}
  </Badge>
{/if}
