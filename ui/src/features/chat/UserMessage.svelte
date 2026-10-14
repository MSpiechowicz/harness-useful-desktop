<script lang="ts">
  import FileText from '@lucide/svelte/icons/file-text';

  import { splitFileNote } from '$lib/chat/attachments';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageItem } from '$lib/chat/types';

  interface Props {
    item: MessageItem;
    /** Thumbnails of the images this window sent with the message; absent for a message loaded from history. */
    images?: string[];
  }

  let { item, images }: Props = $props();

  const message = $derived(splitFileNote(item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n')));
  const text = $derived(message.text);
  const imageTypes = $derived(item.blocks.flatMap((block) => (block.type === 'image' ? [block.mimeType] : [])));
</script>

<div class="flex justify-end">
  <article aria-label={t('chat.user.label')} class="border-card chat-radius flex max-w-[85%] flex-col gap-2 border-accent/30 bg-accent-wash bg-clip-padding px-4 py-2.5 text-base break-words whitespace-pre-wrap text-fg">
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
          <li title={file.path} class="flex max-w-60 items-center gap-2 rounded-lg border border-line-strong bg-surface-1 px-3 py-1.5 text-sm whitespace-normal">
            <FileText size={16} aria-hidden="true" class="shrink-0 text-fg-3" />
            <span class="min-w-0 truncate text-fg">{file.name}</span>
            <span class="shrink-0 text-xs text-fg-3">{file.size}</span>
          </li>
        {/each}
      </ul>
    {/if}
    {#if text !== ''}{text}{/if}
  </article>
</div>
