<script lang="ts">
  import FileText from '@lucide/svelte/icons/file-text';

  import { splitFileNote } from '$lib/chat/attachments';
  import { splitHooks } from '$lib/chat/hook-context';
  import { splitRecall } from '$lib/chat/memory-recall';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageItem } from '$lib/chat/types';

  import CodeBlock from '$lib/components/CodeBlock.svelte';
  import { splitFences } from '$lib/chat/fences';

  import HookCard from './HookCard.svelte';
  import RecallCard from './RecallCard.svelte';

  interface Props {
    item: MessageItem;
    /** Thumbnails of the images this window sent with the message; absent for a message loaded from history. */
    images?: string[];
  }

  let { item, images }: Props = $props();

  // Old transcripts carry a project-memory block, new ones the context hooks added, and a hook may have sent the whole message.
  const split = $derived(splitRecall(item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n')));
  const recall = $derived(split.recall);
  const hooks = $derived(splitHooks(split.text));
  const message = $derived(splitFileNote(hooks.text));
  const text = $derived(message.text);
  const imageTypes = $derived(item.blocks.flatMap((block) => (block.type === 'image' ? [block.mimeType] : [])));
  const parts = $derived(splitFences(text));
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
    <div class="flex justify-end">
      <article aria-label={t('chat.user.label')} class="border-card chat-radius flex max-w-[85%] flex-col gap-2 border-accent/30 bg-accent-wash bg-clip-padding px-4 py-2.5 text-base break-words text-fg">
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
      </article>
    </div>
  {/if}
</div>
