<script lang="ts">
  import File from '@lucide/svelte/icons/file';
  import Folder from '@lucide/svelte/icons/folder';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import { ICON } from '$lib/components/icon';
  import { rise } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeIntake } from '$lib/ipc/bindings/KnowledgeIntake';
  import { formatBytes } from '$lib/utils/format';

  import { folderOf, SKIP_REASONS } from './import-format';

  interface Props {
    intake: KnowledgeIntake;
  }

  let { intake }: Props = $props();

  const entries = $derived([...intake.folders.map((entry) => ({ ...entry, folder: true })), ...intake.files.map((entry) => ({ ...entry, folder: false }))]);
</script>

<div class="flex flex-col gap-3">
  <p class="text-sm font-medium text-fg-2">{[intake.files.length > 0 ? tn('knowledge.import.staged.files', intake.files.length) : '', intake.folders.length > 0 ? tn('knowledge.import.staged.folders', intake.folders.length) : ''].filter(Boolean).join(', ')}</p>

  <ul class="max-h-48 divide-y divide-line overflow-y-auto rounded-lg border border-line" aria-label={t('knowledge.import.staged.label')}>
    {#each entries as entry, index (entry.path)}
      <li in:rise={{ y: 4, delay: Math.min(index, 8) * 30, duration: 200 }} class="flex h-11 items-center gap-3 px-3">
        {#if entry.folder}
          <Folder size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />
        {:else}
          <File size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
        {/if}

        <span class="min-w-0 flex-1 truncate text-sm text-fg" title={entry.path}>{entry.name}</span>
        <span class="hidden min-w-0 max-w-48 truncate text-xs text-fg-3 sm:block" dir="rtl"><bdi>{folderOf(entry.path)}</bdi></span>

        {#if entry.bytes !== null}<span class="tabular shrink-0 text-xs text-fg-3">{formatBytes(entry.bytes)}</span>{/if}
      </li>
    {/each}
  </ul>

  {#if intake.folders.length > 0}
    <p class="text-xs text-fg-3">{t('knowledge.import.folderNote')}</p>
  {/if}

  {#if intake.skipped.length > 0}
    <ul class="flex flex-col gap-1 rounded-md bg-warning-wash px-3 py-2 text-sm text-warning" role="status">
      {#each intake.skipped as skipped (skipped.name + skipped.reason)}
        <li class="flex items-start gap-2"><TriangleAlert size={ICON.sm} class="mt-0.5 shrink-0" aria-hidden="true" /><span class="min-w-0 [overflow-wrap:anywhere]">{t('knowledge.import.skipped', { name: skipped.name, reason: t(SKIP_REASONS[skipped.reason]) })}</span></li>
      {/each}
    </ul>
  {/if}
</div>
