<script lang="ts">
  import { hasMemoryRow } from './memory-rows';
  import MemoryWaiting from './MemoryWaiting.svelte';

  interface Props {
    /** The key of the chat's agent. */
    agentKey: string;
  }

  let { agentKey }: Props = $props();
</script>

{#if hasMemoryRow(agentKey)}
  <MemoryWaiting {agentKey} />
{/if}
