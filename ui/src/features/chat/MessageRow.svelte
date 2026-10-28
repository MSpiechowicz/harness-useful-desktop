<script lang="ts">
  import type { MemoryUseIndex } from '$lib/chat/memory-use';
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
    /** What each reply did with the notes its searches found (see `buildMemoryUse`). */
    memoryUse?: MemoryUseIndex;
  }

  let { item, chat, sentImages, memoryUse }: Props = $props();

  const active = $derived(chat.phase === 'streaming');
</script>

{#if item.kind === 'message'}
  {#if item.role === 'user'}
    <UserMessage {item} images={sentImages?.[item.key]} />
  {:else if item.role === 'assistant'}
    <AssistantMessage {item} toolCalls={chat.toolCalls} {active} subagents={chat.subagents} {memoryUse} />
  {:else}
    <SystemMessage {item} />
  {/if}
{:else if item.kind === 'tool'}
  {@const call = chat.toolCalls[item.toolCallId]}
  {#if call}
    <ChatToolCard {call} {active} subagents={chat.subagents} {memoryUse} />
  {/if}
{:else if item.kind === 'memory'}
  <MemoryChatRow agentKey={chat.key} />
{:else}
  <TranscriptNote {item} />
{/if}
