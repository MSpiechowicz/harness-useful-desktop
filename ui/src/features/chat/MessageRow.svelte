<script lang="ts">
  import type { ChatState, TranscriptItem } from '$lib/chat/types';

  import AssistantMessage from './AssistantMessage.svelte';
  import SystemMessage from './SystemMessage.svelte';
  import ToolCard from './ToolCard.svelte';
  import TranscriptNote from './TranscriptNote.svelte';
  import UserMessage from './UserMessage.svelte';

  interface Props {
    item: TranscriptItem;
    chat: ChatState;
    /** Thumbnails of the images sent with each user message, by message key. */
    sentImages?: Record<string, string[]>;
  }

  let { item, chat, sentImages }: Props = $props();

  const active = $derived(chat.phase === 'streaming');
</script>

{#if item.kind === 'message'}
  {#if item.role === 'user'}
    <UserMessage {item} images={sentImages?.[item.key]} />
  {:else if item.role === 'assistant'}
    <AssistantMessage {item} toolCalls={chat.toolCalls} {active} subagents={chat.subagents} />
  {:else}
    <SystemMessage {item} />
  {/if}
{:else if item.kind === 'tool'}
  {@const call = chat.toolCalls[item.toolCallId]}
  {#if call}
    <ToolCard {call} {active} subagents={chat.subagents} />
  {/if}
{:else}
  <TranscriptNote {item} />
{/if}
