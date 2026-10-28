<script lang="ts">
  import type { ComponentProps } from 'svelte';

  import { memoryToolName } from '$lib/chat/memory-tools';
  import type { MemoryUseIndex } from '$lib/chat/memory-use';

  import MemoryFeedbackCard from './MemoryFeedbackCard.svelte';
  import MemoryReadCard from './MemoryReadCard.svelte';
  import MemorySearchCard from './MemorySearchCard.svelte';
  import ToolCard from './ToolCard.svelte';

  /** A tool call as `ToolCard` takes it, plus what each reply did with the notes its searches found. */
  type Props = ComponentProps<typeof ToolCard> & { memoryUse?: MemoryUseIndex };

  let { call, active, subagents = {}, memoryUse }: Props = $props();

  const memoryTool = $derived(memoryToolName(call.name));
</script>

<!-- One place that decides how a tool call looks: the three project-memory tools that tell something to the user have their
     own card, every other call (the other memory tools included) is the generic one. -->
{#if memoryTool === 'note_search'}
  <MemorySearchCard {call} {active} use={memoryUse?.get(call.id)} />
{:else if memoryTool === 'note_get'}
  <MemoryReadCard {call} {active} />
{:else if memoryTool === 'note_feedback'}
  <MemoryFeedbackCard {call} {active} found={memoryUse?.rated?.get(call.id)} />
{:else}
  <ToolCard {call} {active} {subagents} />
{/if}
