<script lang="ts">
  import FileText from '@lucide/svelte/icons/file-text';
  import Undo2 from '@lucide/svelte/icons/undo-2';

  import { splitFileNote } from '$lib/chat/attachments';
  import { splitHooks } from '$lib/chat/hook-context';
  import { splitRecall } from '$lib/chat/memory-recall';
  import type { KnowledgeUseIndex } from '$lib/chat/knowledge-use';
  import type { MemoryUseIndex } from '$lib/chat/memory-use';
  import { messageRecall, recallCallId } from '$lib/chat/recall-rows';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageItem } from '$lib/chat/types';

  import IconButton from '$lib/components/IconButton.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import CodeBlock from '$lib/components/CodeBlock.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import { splitFences } from '$lib/chat/fences';

  import { sentAt } from './user-meta';
  import HookCard from './HookCard.svelte';
  import KnowledgeRecallCard from './KnowledgeRecallCard.svelte';
  import MemoryRecallCard from './MemoryRecallCard.svelte';
  import RecallCard from './RecallCard.svelte';

  interface Props {
    item: MessageItem;
    /** Thumbnails of the images this window sent with the message; absent for a message loaded from history. */
    images?: string[];
    /** What the reply to this message did with the notes and documents the app found for it (see `buildMemoryUse`, `buildKnowledgeUse`). */
    memoryUse?: MemoryUseIndex;
    knowledgeUse?: KnowledgeUseIndex;
    /** Puts the user's words back in the composer; without it (a transcript with no composer) there is no Use again. */
    onUseAgain?: (text: string) => void;
  }

  let { item, images, memoryUse, knowledgeUse, onUseAgain }: Props = $props();

  // Old transcripts carry a project-memory block, new ones the context hooks added, and a hook may have sent the whole message.
  const split = $derived(splitRecall(item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n')));
  const recall = $derived(split.recall);
  const hooks = $derived(splitHooks(split.text));
  const message = $derived(splitFileNote(hooks.text));
  const text = $derived(message.text);
  const imageTypes = $derived(item.blocks.flatMap((block) => (block.type === 'image' ? [block.mimeType] : [])));
  const parts = $derived(splitFences(text));
  /** What the app found for the message, as the first search of its reply. */
  const found = $derived(messageRecall(item));
  const memoryId = $derived(recallCallId(item.key, 'memory'));
  const knowledgeId = $derived(recallCallId(item.key, 'knowledge'));

  const sent = $derived(sentAt(item.meta?.timestamp));
  const hasWords = $derived(text.trim() !== '');
  /** Shown before the harness echoed it: the time is not known yet. */
  const sending = $derived(item.send?.status === 'sending');
</script>

<div class="flex flex-col gap-3">
  {#if recall}
    <RecallCard {recall} />
  {/if}
  {#if hooks.context.length > 0}
    <HookCard hooks={hooks.context} kind="context" />
  {/if}
  {#if hooks.followUp.length > 0}
    <HookCard hooks={hooks.followUp} kind="followUp" />
  {/if}
  <!-- A message a hook sent is only its card: there are no words of the user's to show. -->
  {#if hooks.followUp.length === 0}
    <div class="flex flex-col items-end gap-1">
      <article aria-label={t('chat.user.label')} aria-busy={sending || undefined} class="border-card chat-radius flex max-w-[85%] flex-col overflow-hidden border-accent/30 bg-accent-wash bg-clip-padding text-base break-words text-fg">
        <div class="flex flex-col gap-2 px-4 py-2.5">
          {#if images && images.length > 0}
            <ul class="flex flex-wrap gap-2" aria-label={t('chat.user.images')}>
              {#each images as url, index (index)}
                <li><img src={url} alt={t('chat.user.image', { n: index + 1 })} class="max-h-40 max-w-full rounded-md object-cover" /></li>
              {/each}
            </ul>
          {:else if imageTypes.length > 0}
            <ul class="flex flex-wrap gap-2 text-sm text-fg-3" aria-label={t('chat.user.images')}>
              {#each imageTypes as type, index (index)}
                <li>{t('chat.assistant.image', { type })}</li>
              {/each}
            </ul>
          {/if}
          {#if message.files.length > 0}
            <ul class="flex flex-wrap gap-2" aria-label={t('chat.user.files')}>
              {#each message.files as file (file.path)}
                <li title={file.path} class="flex max-w-60 items-center gap-2 rounded-md border border-line-strong bg-surface-1 px-3 py-1.5 text-sm whitespace-normal">
                  <FileText size={16} aria-hidden="true" class="shrink-0 text-fg-3" />
                  <span class="min-w-0 truncate text-fg">{file.name}</span>
                  <span class="shrink-0 text-xs text-fg-3">{file.size}</span>
                </li>
              {/each}
            </ul>
          {/if}
          {#each parts as part, index (index)}
            {#if part.type === 'code'}
              <CodeBlock code={part.code} lang={part.lang} />
            {:else}
              <div class="whitespace-pre-wrap">{part.text}</div>
            {/if}
          {/each}
        </div>
      </article>

      <!-- A quiet line under the bubble: when it was sent, and the two things to do with the user's own words (never the hook context or the file notes). -->
      {#if sending || sent !== '' || hasWords}
        <div role="group" aria-label={t('chat.user.details')} class="flex items-center gap-0.5 pr-1 text-xs text-fg-3">
          {#if sending}
            <Spinner size={14} />
            <span class="mr-1.5 ml-1 shrink-0">{t('chat.user.sending')}</span>
          {:else if sent !== ''}
            <span class="mr-1.5 shrink-0">{sent}</span>
          {/if}
          {#if hasWords}
            <CopyButton control text={text} label={t('chat.user.copy')} />
            {#if onUseAgain}
              <IconButton icon={Undo2} label={t('chat.user.useAgain')} disabled={sending} onclick={() => onUseAgain(text)} />
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <!-- The notes and documents the app found are the first thing the reply did, so they follow the message and come before the agent's own cards. -->
  {#if found.memory}
    <MemoryRecallCard id={memoryId} recall={found.memory} use={memoryUse?.get(memoryId)} />
  {/if}
  {#if found.knowledge}
    <KnowledgeRecallCard id={knowledgeId} recall={found.knowledge} use={knowledgeUse?.get(knowledgeId)} />
  {/if}
</div>
