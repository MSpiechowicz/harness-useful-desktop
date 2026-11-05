<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Markdown from '$lib/components/Markdown.svelte';
  import type { KnowledgeUseIndex } from '$lib/chat/knowledge-use';
  import type { MemoryUseIndex } from '$lib/chat/memory-use';
  import { splitStepStatus } from '$lib/chat/step-status';
  import type { MessageItem, ToolCallState, TranscriptItem } from '$lib/chat/types';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ChatToolCard from './ChatToolCard.svelte';
  import MessageMeta from './MessageMeta.svelte';
  import StepStatusCard from './StepStatusCard.svelte';
  import ThinkingBlock from './ThinkingBlock.svelte';

  interface Props {
    item: MessageItem;
    toolCalls: Record<string, ToolCallState>;
    /** The agent is still working (a tool that never started is "waiting", not "not run"). */
    active: boolean;
    /** The session's subagents, for the `task` calls in this message. */
    subagents?: Record<string, Subagent>;
    /** What each reply did with the notes and documents its searches found, for the project-memory and project-documentation cards. */
    memoryUse?: MemoryUseIndex;
    knowledgeUse?: KnowledgeUseIndex;
    /** The transcript around the message, for what its status card reports. Just the message when it is shown alone. */
    items?: TranscriptItem[];
  }

  let { item, toolCalls, active, subagents = {}, memoryUse, knowledgeUse, items }: Props = $props();

  const lastIndex = $derived(item.blocks.length - 1);
  const empty = $derived(item.blocks.length === 0);
  /** The thought process and the cost line can be hidden from the chat header or Settings. */
  const showThinking = $derived(prefs.value?.showThinking ?? true);
  const showUsage = $derived(prefs.value?.showMessageUsage ?? true);
</script>

<article class="flex flex-col gap-3" aria-label={t('chat.assistant.label')} aria-busy={item.streaming}>
  {#each item.blocks as block, index (index)}
    {#if block.type === 'thinking'}
      {#if showThinking}<ThinkingBlock text={block.text} redacted={block.redacted} streaming={item.streaming && index === lastIndex} />{/if}
    {:else if block.type === 'text'}
      {@const live = item.streaming && index === lastIndex}
      {@const status = splitStepStatus(block.text, live)}
      {#if status}
        {#if status.before}<div><Markdown source={status.before} /></div>{/if}
        {#if status.table}<StepStatusCard table={status.table} messageKey={item.key} items={items ?? [item]} {toolCalls} {memoryUse} {knowledgeUse} />{/if}
        {#if status.after || live}
          <div>
            <Markdown source={status.after} streaming={live} />
            {#if live}<span class="caret" aria-hidden="true"></span>{/if}
          </div>
        {/if}
      {:else}
        <div>
          <Markdown source={block.text} streaming={live} />
          {#if live}<span class="caret" aria-hidden="true"></span>{/if}
        </div>
      {/if}
    {:else if block.type === 'tool'}
      {@const call = toolCalls[block.toolCallId]}
      {#if call}
        <ChatToolCard {call} {active} {subagents} {memoryUse} {knowledgeUse} />
      {/if}
    {:else if block.type === 'image'}
      <p class="text-sm text-fg-3">{t('chat.assistant.image', { type: block.mimeType })}</p>
    {:else}
      <p class="text-sm text-fg-3">[{block.kind}]</p>
    {/if}
  {/each}

  {#if empty && item.streaming}
    <p class="text-sm text-fg-3" role="status"><span class="sheen">{t('chat.assistant.working')}</span></p>
  {/if}

  {#if item.meta?.errorMessage}
    <!-- Laid out like the card rows above it (same height, icon column and padding), so the turn reads as one list. -->
    <p class="chat-radius grid min-h-11 grid-cols-[28px_minmax(0,1fr)] items-center gap-x-3 border border-danger/30 bg-danger-wash py-2 pr-3.5 pl-[18px] text-sm text-fg" role="alert">
      <span class="flex justify-center"><CircleAlert size={16} class="text-danger" aria-hidden="true" /></span>
      <span class="break-words">{item.meta.errorMessage}</span>
    </p>
  {/if}

  {#if item.meta && !item.streaming && showUsage}
    <MessageMeta meta={item.meta} />
  {/if}
</article>
