<script lang="ts">
  import { tn } from '$lib/i18n/i18n.svelte';
  import type { MessageMeta } from '$lib/chat/types';
  import { formatCost } from '$lib/utils/format';

  interface Props {
    meta: MessageMeta;
  }

  let { meta }: Props = $props();

  const parts = $derived(
    [
      meta.model,
      meta.usage ? tn('chat.meta.tokens', meta.usage.total) : null,
      meta.costUsd !== null ? formatCost(meta.costUsd) : null,
    ].filter((part): part is string => part !== null),
  );
</script>

{#if parts.length > 0}
  <p class="tabular text-xs text-fg-3">{parts.join(' · ')}</p>
{/if}
