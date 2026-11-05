<script lang="ts">
  import type { KnowledgeUseIndex } from '$lib/chat/knowledge-use';
  import type { MemoryUseIndex } from '$lib/chat/memory-use';
  import { pendingKey } from '$lib/chat/pending-send';
  import type { ChatState, TranscriptItem } from '$lib/chat/types';

  import AssistantMessage from './AssistantMessage.svelte';
  import MemoryChatRow from './MemoryChatRow.svelte';
  import ChatToolCard from './ChatToolCard.svelte';
  import SystemMessage from './SystemMessage.svelte';
  import TranscriptNote from './TranscriptNote.svelte';
  import UserMessage from './UserMessage.svelte';

  interface Props {
    item: TranscriptItem;
    chat: ChatState;
    /** Thumbnails of the images sent with each user message, by message key. */
    sentImages?: Record<string, string[]>;
    /** What each reply did with the notes and documents its searches found (see `buildMemoryUse`, `buildKnowledgeUse`). */
    memoryUse?: MemoryUseIndex;
    knowledgeUse?: KnowledgeUseIndex;
    /** Puts a message of the user back in the composer; absent in a transcript that has no composer. */
    onUseAgain?: (text: string) => void;
  }

  let { item, chat, sentImages, memoryUse, knowledgeUse, onUseAgain }: Props = $props();

  const active = $derived(chat.phase === 'streaming');
</script>

{#if item.kind === 'message'}
  {#if item.role === 'user'}
    <UserMessage {item} images={sentImages?.[item.key] ?? (item.send ? sentImages?.[pendingKey(item.send.id)] : undefined)} {memoryUse} {knowledgeUse} {onUseAgain} />
  {:else if item.role === 'assistant'}
    <AssistantMessage {item} toolCalls={chat.toolCalls} {active} subagents={chat.subagents} {memoryUse} {knowledgeUse} items={chat.items} />
  {:else}
    <SystemMessage {item} />
  {/if}
{:else if item.kind === 'tool'}
  {@const call = chat.toolCalls[item.toolCallId]}
  {#if call}
    <ChatToolCard {call} {active} subagents={chat.subagents} {memoryUse} {knowledgeUse} />
  {/if}
{:else if item.kind === 'memory'}
  <MemoryChatRow agentKey={chat.key} />
{:else}
  <TranscriptNote {item} />
{/if}
