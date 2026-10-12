<script lang="ts">
  import FileDiff from '@lucide/svelte/icons/file-diff';
  import FileX from '@lucide/svelte/icons/file-x';
  import FolderSearch from '@lucide/svelte/icons/folder-search';

  import { t } from '$lib/i18n/i18n.svelte';
  import type { FileDiff as FileDiffData } from '$lib/ipc/bindings/FileDiff';

  import Button from './Button.svelte';
  import { ICON } from './icon';

  interface Props {
    diff: FileDiffData;
    /** Opens the file in the file manager; offered for binary files, which the diff cannot show. */
    onreveal?: () => void;
  }

  let { diff, onreveal }: Props = $props();

  const notice = $derived.by(() => {
    if (diff.isBinary) {
      return { icon: FileX, title: t('components.diff.binary.title'), body: t('components.diff.binary.body') };
    }

    if (diff.truncated) {
      return { icon: FileX, title: t('components.diff.hidden.title'), body: t('components.diff.hidden.body') };
    }

    return { icon: FileDiff, title: t('components.diff.unchanged.title'), body: t('components.diff.unchanged.body') };
  });
</script>

<!-- Sits in the upper third, where the eye lands, instead of floating in the middle of a large empty pane. -->
<div class="flex size-full justify-center overflow-y-auto px-8 pt-[min(18vh,9rem)]" role="status">
  <div class="flex h-fit max-w-sm flex-col items-center gap-4 text-center">
    <span class="grid size-12 place-items-center rounded-lg border border-line-strong bg-surface-2 text-fg-2 shadow-float">
      <notice.icon size={ICON.lg} strokeWidth={1.6} aria-hidden="true" />
    </span>

    <div class="flex flex-col gap-1">
      <p class="text-lg font-semibold text-fg">{notice.title}</p>
      <p class="text-base text-fg-2">{notice.body}</p>
    </div>

    {#if diff.isBinary && onreveal}
      <Button onclick={onreveal}>
        {#snippet leading()}<FolderSearch size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('components.diff.reveal')}
      </Button>
    {/if}
  </div>
</div>
