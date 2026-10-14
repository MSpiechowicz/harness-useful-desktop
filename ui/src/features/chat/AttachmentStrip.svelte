<script lang="ts">
  import FileText from '@lucide/svelte/icons/file-text';
  import X from '@lucide/svelte/icons/x';

  import { formatBytes } from '$lib/chat/attachments';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const images = $derived(session.attached.images);
  const files = $derived(session.attached.files);
  /** The model is known and takes no images, so the agent would never see them. */
  const blind = $derived.by(() => {
    const input = session.chat.session?.model?.input;

    return images.length > 0 && input !== undefined && input.length > 0 && !input.includes('image');
  });
</script>

{#snippet remove(name: string, onclick: () => void)}
  <button
    type="button"
    aria-label={t('chat.attach.remove', { name })}
    {onclick}
    class="absolute -top-1.5 -right-1.5 grid size-5 place-items-center rounded-full bg-surface-3 text-fg-2 shadow-float outline-none hover:text-fg focus-visible:ring-2 focus-visible:ring-accent"
  >
    <X size={12} aria-hidden="true" />
  </button>
{/snippet}

{#if images.length > 0 || files.length > 0}
  <ul class="flex flex-wrap gap-2 px-3 pt-3" aria-label={t('chat.attach.list')}>
    {#each images as image (image.id)}
      <li class="relative">
        <img src={image.url} alt={image.name} class="size-16 rounded-lg border border-line-strong object-cover" />
        {@render remove(image.name, () => session.attached.removeImage(image.id))}
      </li>
    {/each}
    {#each files as file (file.path)}
      <li class="relative" title={file.path}>
        <div class="flex h-16 max-w-56 items-center gap-2 rounded-lg border border-line-strong bg-surface-2 px-3">
          <FileText size={18} aria-hidden="true" class="shrink-0 text-fg-3" />
          <div class="min-w-0">
            <p class="truncate text-sm text-fg">{file.name}</p>
            <p class="text-xs text-fg-3">{formatBytes(file.bytes)}</p>
          </div>
        </div>
        {@render remove(file.name, () => session.attached.removeFile(file.path))}
      </li>
    {/each}
  </ul>
  {#if blind}
    <p class="px-4 pt-2 text-xs text-warning">{t('chat.attach.blind')}</p>
  {/if}
{/if}
