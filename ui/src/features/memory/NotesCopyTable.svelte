<script lang="ts">
  import Checkbox from '$lib/components/Checkbox.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
  import type { NotesCopyRow } from '$lib/ipc/bindings/NotesCopyRow';
  import { formatDateTime, relativeTime } from '$lib/utils/format';

  import { COPY_PAGE_SIZE } from './notes-copy-state.svelte';
  import { COPY_TEXT } from './notes-copy-text';
  import { noteScopeText } from './note-format';
  import { spelledOut } from './script-format';

  interface Props {
    /** What the rows are: the notes or the scripts. */
    kind: MemoryCopyKind;
    /** The rows of the page on screen, in the server's order: grouped by repository. */
    rows: readonly NotesCopyRow[];
    page: number;
    /** How many rows there are in all pages. */
    total: number;
    /** The page is being read. */
    busy: boolean;
    /** The rows are the entries that cannot be copied: each shows its reason and has no tick. */
    blocked?: boolean;
    picked?: (localId: number) => boolean;
    ontoggle?: (localId: number, on: boolean) => void;
    onpage: (page: number) => void;
  }

  let { kind, rows, page, total, busy, blocked = false, picked = () => false, ontoggle = () => {}, onpage }: Props = $props();

  const text = $derived(COPY_TEXT[kind]);

  const COLUMNS = $derived(blocked ? (['title', 'reason'] as const) : (['pick', 'title', 'local', 'team'] as const));
  const CELL = 'border-b border-line/60 px-3 py-2 align-middle';

  /** A row opens a group when its repository differs from the row above. */
  function opensGroup(index: number): boolean {
    return index === 0 || rows[index].repo !== rows[index - 1].repo;
  }
</script>

<!-- The entries that differ from the remote copy, or the ones that cannot be copied, a page of the server's plan at a time and grouped by repository. An entry's tick means its version replaces the remote one. A blocked entry has no tick, only its reason. -->
<div class="@container flex min-w-0 flex-col overflow-clip rounded-md border border-line" aria-busy={busy}>
  <div class="max-h-[40vh] overflow-y-auto overscroll-contain">
    <table aria-label={t(blocked ? text.blockedTableLabel : text.tableLabel)} class="w-full table-fixed border-separate border-spacing-0 text-sm">
      <thead class="sticky top-0 z-10 bg-surface-1">
        <tr>
          {#each COLUMNS as column (column)}
            <th
              scope="col"
              class="h-9 border-b border-line px-3 text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase {column === 'pick' ? 'w-20' : ''} {column === 'title' && blocked ? 'w-2/5' : ''} {column === 'local' || column === 'team' ? 'hidden w-36 @lg:table-cell' : ''}"
            >
              <span class="truncate">{column === 'title' ? t(text.columnTitle) : t(`memory.copy.column.${column}`)}</span>
            </th>
          {/each}
        </tr>
      </thead>

      <tbody class="transition-opacity duration-[var(--dur-fast)] {busy ? 'opacity-60' : ''}">
        {#each rows as row, index (row.localId)}
          {#if opensGroup(index)}
            <tr><th scope="rowgroup" colspan={COLUMNS.length} class="h-9 border-b border-line bg-surface-2 px-3 text-left text-xs font-medium text-fg-2" title={row.repo ?? undefined}>{row.scope === 'global' ? t(text.groupGlobal) : noteScopeText(row)}</th></tr>
          {/if}

          <tr class="h-11">
            {#if blocked}
              <td class={CELL}><div class="[overflow-wrap:anywhere] text-fg">{spelledOut(row.title)}</div></td>
              <td class={CELL}><div class="[overflow-wrap:anywhere] text-warning">{row.failure}</div></td>
            {:else}
              <td class="{CELL} py-0"><Checkbox checked={picked(row.localId)} label={t('memory.copy.row.pick', { title: spelledOut(row.title) })} onchange={(on) => ontoggle(row.localId, on)} /></td>
              <td class={CELL}><div class="[overflow-wrap:anywhere] text-fg">{spelledOut(row.title)}</div></td>
              {#each [row.localUpdatedAt, row.teamUpdatedAt] as moment, index (index)}
                <td class="{CELL} tabular hidden text-fg-3 @lg:table-cell" title={moment ? formatDateTime(moment) : undefined}>{moment ? relativeTime(moment) : '–'}</td>
              {/each}
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if total > COPY_PAGE_SIZE}
    <Pagination {page} pageSize={COPY_PAGE_SIZE} {total} {onpage} />
  {/if}
</div>
