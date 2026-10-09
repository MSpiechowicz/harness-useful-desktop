<script lang="ts">
  import type { MessageMeta } from '$lib/chat/types';

  interface Props {
    meta: MessageMeta;
  }

  let { meta }: Props = $props();

  const parts = $derived(
    [
      meta.model,
      meta.usage ? `${meta.usage.total.toLocaleString()} tokens` : null,
      meta.costUsd !== null ? `$${meta.costUsd.toFixed(4)}` : null,
    ].filter((part): part is string => part !== null),
  );
</script>

{#if parts.length > 0}
  <p class="tabular text-xs text-fg-3">{parts.join(' · ')}</p>
{/if}
