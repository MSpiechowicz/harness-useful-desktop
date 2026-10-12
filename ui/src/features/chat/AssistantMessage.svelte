<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Markdown from '$lib/components/Markdown.svelte';
  import type { MessageItem, ToolCallState } from '$lib/chat/types';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import MessageMeta from './MessageMeta.svelte';
  import ThinkingBlock from './ThinkingBlock.svelte';
  import ToolCard from './ToolCard.svelte';

  interface Props {
    item: MessageItem;
    toolCalls: Record<string, ToolCallState>;
    /** The agent is still working (a tool that never started is "waiting", not "not run"). */
    active: boolean;
    /** The session's subagents, for the `task` calls in this message. */
    subagents?: Record<string, Subagent>;
  }

  let { item, toolCalls, active, subagents = {} }: Props = $props();

  const lastIndex = $derived(item.blocks.length - 1);
  const empty = $derived(item.blocks.length === 0);
  /** The thought process can be hidden from the chat header or Settings. */
  const showThinking = $derived(prefs.value?.showThinking ?? true);
</script>

<article class="flex flex-col gap-3" aria-label={t('chat.assistant.label')} aria-busy={item.streaming}>
  {#each item.blocks as block, index (index)}
    {#if block.type === 'thinking'}
      {#if showThinking}<ThinkingBlock text={block.text} redacted={block.redacted} streaming={item.streaming && index === lastIndex} />{/if}
    {:else if block.type === 'text'}
      <div>
        <Markdown source={block.text} streaming={item.streaming && index === lastIndex} />
        {#if item.streaming && index === lastIndex}<span class="caret" aria-hidden="true"></span>{/if}
      </div>
    {:else if block.type === 'tool'}
      {@const call = toolCalls[block.toolCallId]}
      {#if call}
        <ToolCard {call} {active} {subagents} />
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
    <p class="flex items-start gap-2 rounded-md border border-danger/30 bg-danger-wash px-3 py-2 text-sm text-fg" role="alert">
      <CircleAlert size={16} class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
      <span class="break-words">{item.meta.errorMessage}</span>
    </p>
  {/if}

  {#if item.meta && !item.streaming}
    <MessageMeta meta={item.meta} />
  {/if}
</article>
