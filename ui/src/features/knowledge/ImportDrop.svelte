<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import FileUp from '@lucide/svelte/icons/file-up';
  import Upload from '@lucide/svelte/icons/upload';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeImport as flow } from '$lib/stores/knowledgeImport.svelte';
</script>

<!-- The window takes a drop anywhere (see the overlay of the view): this says so, and the buttons are the way for a pointer-free hand. -->
<div class="flex flex-col items-center gap-5 rounded-lg border border-dashed border-line-strong bg-surface-2 px-6 py-10 text-center">
  <span class="grid size-12 place-items-center rounded-lg border border-line-strong bg-surface-3 text-accent-text"><Upload size={ICON.lg} strokeWidth={1.6} aria-hidden="true" /></span>

  <div class="flex max-w-sm flex-col gap-1">
    <p class="text-base text-fg">{t('knowledge.import.drop.title')}</p>
    <p class="text-sm text-fg-3">{t('knowledge.import.drop.hint')}</p>
  </div>

  <div class="flex flex-wrap items-center justify-center gap-2">
    <Button variant="primary" loading={flow.picking} onclick={() => void flow.pick('files')}>
      {#snippet leading()}<FileUp size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('knowledge.import.chooseFiles')}
    </Button>

    <Button disabled={flow.picking} onclick={() => void flow.pick('folder')}>
      {#snippet leading()}<FolderOpen size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('knowledge.import.chooseFolder')}
    </Button>
  </div>
</div>
