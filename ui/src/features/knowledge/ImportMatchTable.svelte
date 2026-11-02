<script lang="ts">
  import File from '@lucide/svelte/icons/file';
  import Folder from '@lucide/svelte/icons/folder';

  import { flip } from 'svelte/animate';

  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { rise, scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeMatch } from '$lib/ipc/bindings/KnowledgeMatch';
  import { authorLine } from '$lib/utils/author-format';
  import { formatBytes } from '$lib/utils/format';
  import { canReplace, inOtherPlace, type Landing, type MatchChoices, type MatchRow, updates } from '$lib/utils/knowledge-match';

  import ImportMatchAction from './ImportMatchAction.svelte';
  import { folderLabel } from './import-format';
  import { placeLabel } from './knowledge-format';
  import { placeLabelOf, REASON_LABELS } from './knowledge-match-text';

  interface Props {
    rows: MatchRow[];
    /** The comparison has not answered yet: the Existing column is a skeleton. */
    pending: boolean;
    choices: MatchChoices;
    /** Where the import files its documents. */
    landing: Landing;
    onchange: (path: string, update: boolean) => void;
  }

  let { rows, pending, choices, landing, onchange }: Props = $props();

  const COLUMNS = 'grid-cols-[minmax(0,5fr)_minmax(0,6fr)_9.5rem]';

  /** "Remote knowledge · added by Anna": where the document is kept, then who added it when that is known. */
  function existingLine(match: KnowledgeMatch): string {
    return [placeLabelOf(match.place), ...(match.addedBy ? [t('knowledge.match.addedBy', { name: authorLine(match.addedBy) })] : [])].join(' · ');
  }
</script>

<!-- The files and what each would do. The header sticks while a long list scrolls. -->
<div role="table" aria-label={t('knowledge.match.label')} aria-busy={pending} class="max-h-96 overflow-y-auto rounded-lg border border-line">
  <div role="row" class="sticky top-0 z-10 grid items-center gap-x-3 border-b border-line bg-surface-2 px-3 py-2.5 text-xs font-medium tracking-wider text-fg-3 uppercase {COLUMNS}">
    <span role="columnheader">{t('knowledge.match.yourFile')}</span>
    <span role="columnheader">{t('knowledge.match.existing')}</span>
    <span role="columnheader">{t('knowledge.match.action')}</span>
  </div>

  {#each rows as row, index (row.path)}
    {@const Icon = row.folder ? Folder : File}
    {@const meta = [folderLabel(row.path), ...(row.bytes === null ? [] : [formatBytes(row.bytes)])].filter(Boolean).join(' · ')}

    <div role="row" in:rise={{ y: 4, delay: Math.min(index, 8) * 30, duration: 200 }} animate:flip={{ duration: scaled(220) }} class="grid min-h-14 items-center gap-x-3 border-t border-line px-3 py-2 first:border-t-0 {COLUMNS}">
      <div role="cell" class="flex min-w-0 items-center gap-2.5">
        <Icon size={ICON.sm} class="shrink-0 {row.folder ? 'text-accent-text' : 'text-fg-3'}" aria-hidden="true" />
        <div class="min-w-0">
          <p class="truncate text-sm text-fg" title={row.path}>{row.name}</p>
          {#if meta}<p class="truncate text-left text-xs text-fg-3" dir="rtl"><bdi>{meta}</bdi></p>{/if}
        </div>
      </div>

      <div role="cell" class="min-w-0" title={row.match ? t(REASON_LABELS[row.match.reason]) : undefined}>
        {#if row.match}
          <p class="truncate text-sm text-fg">{row.match.title}</p>
          <p class="truncate text-xs text-fg-3">{existingLine(row.match)}</p>
        {:else if row.error}
          <p class="truncate text-xs text-fg-3" title={row.error}>{t('knowledge.match.error', { reason: row.error })}</p>
        {:else if pending}
          <div class="flex flex-col gap-1.5" aria-hidden="true">
            <Skeleton width="60%" height="0.875rem" />
            <Skeleton width="38%" height="0.75rem" />
          </div>
        {/if}
      </div>

      <div role="cell" class="min-w-0">
        {#if row.match && canReplace(row, landing)}
          <ImportMatchAction update={updates(row, choices, landing)} name={row.name} title={row.match.title} onchange={(next) => onchange(row.path, next)} />
        {:else if !pending}
          <!-- The server refuses to replace a document of the other place, so the file is added as new. The tooltip says where the match is kept. -->
          {@const kept = row.match && landing !== 'unknown' && inOtherPlace(row, landing) ? t('knowledge.match.keptElsewhere', { place: placeLabel(row.match.place) }) : undefined}

          <span class="px-3 text-sm text-fg-3" title={kept ?? (row.fromFolder && row.match ? t('knowledge.match.folderFile') : undefined)}>{t('knowledge.match.addNew')}</span>
        {/if}
      </div>
    </div>
  {/each}
</div>
