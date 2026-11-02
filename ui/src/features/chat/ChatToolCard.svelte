<script lang="ts">
  import type { ComponentProps } from 'svelte';

  import { knowledgeToolName } from '$lib/chat/knowledge-tools';
  import type { KnowledgeUseIndex } from '$lib/chat/knowledge-use';
  import { memoryToolName } from '$lib/chat/memory-tools';
  import type { MemoryUseIndex } from '$lib/chat/memory-use';

  import KnowledgeFeedbackCard from './KnowledgeFeedbackCard.svelte';
  import KnowledgeSearchCard from './KnowledgeSearchCard.svelte';
  import MemoryFeedbackCard from './MemoryFeedbackCard.svelte';
  import MemoryReadCard from './MemoryReadCard.svelte';
  import MemorySearchCard from './MemorySearchCard.svelte';
  import ToolCard from './ToolCard.svelte';

  /** A tool call as `ToolCard` takes it, plus what each reply did with the notes and documents its searches found. */
  type Props = ComponentProps<typeof ToolCard> & { memoryUse?: MemoryUseIndex; knowledgeUse?: KnowledgeUseIndex };

  let { call, active, subagents = {}, memoryUse, knowledgeUse }: Props = $props();

  const memoryTool = $derived(memoryToolName(call.name));
  const knowledgeTool = $derived(knowledgeToolName(call.name));
</script>

<!-- One place that decides how a tool call looks: the project-memory and project-documentation tools that tell something to the user
     have their own card, every other call (the other tools of those servers included) is the generic one. -->
{#if memoryTool === 'note_search'}
  <MemorySearchCard {call} {active} use={memoryUse?.get(call.id)} />
{:else if memoryTool === 'note_get'}
  <MemoryReadCard {call} {active} />
{:else if memoryTool === 'note_feedback'}
  <MemoryFeedbackCard {call} {active} found={memoryUse?.rated?.get(call.id)} />
{:else if knowledgeTool === 'knowledge_search'}
  <KnowledgeSearchCard {call} {active} use={knowledgeUse?.get(call.id)} />
{:else if knowledgeTool === 'knowledge_feedback'}
  <KnowledgeFeedbackCard {call} {active} found={knowledgeUse?.rated?.get(call.id)} />
{:else}
  <ToolCard {call} {active} {subagents} />
{/if}
