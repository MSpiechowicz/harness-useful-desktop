<script lang="ts">
  import Checkbox from '$lib/components/Checkbox.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import { minTableWidth } from '$lib/components/table';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
  import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';
  import type { NotesCopyRow } from '$lib/ipc/bindings/NotesCopyRow';
  import { formatDateTime, relativeTime } from '$lib/utils/format';

  import { COPY_PAGE_SIZE, tickId } from './notes-copy-state.svelte';
  import { COPY_TEXT, pickText, rowStatusKey } from './notes-copy-text';
  import { noteScopeText } from './note-format';
  import { spelledOut } from './script-format';

  interface Props {
    /** What the rows are: the notes or the scripts. */
    kind: MemoryCopyKind;
    /** Which way the copy goes: when the user picks (to the remote or to this computer) the ticks pick entries to copy, and every row says where it stands. */
    mode?: NotesCopyMode;
    /** The rows of the page on screen, in the server's order: grouped by repository. */
    rows: readonly NotesCopyRow[];
    page: number;
    /** How many rows there are in all pages. */
    total: number;
    /** The page is being read. */
    busy: boolean;
    /** The rows are the entries that cannot be copied: each shows its reason and has no tick. */
    blocked?: boolean;
    picked?: (id: number) => boolean;
    ontoggle?: (id: number, on: boolean) => void;
    onpage: (page: number) => void;
  }

  let { kind, mode = 'stranded', rows, page, total, busy, blocked = false, picked = () => false, ontoggle = () => {}, onpage }: Props = $props();

  const text = $derived(COPY_TEXT[kind]);
  const picking = $derived(mode !== 'stranded');

  const COLUMNS = $derived(blocked ? (['title', 'reason'] as const) : picking ? (['pick', 'title', 'local', 'team', 'status'] as const) : (['pick', 'title', 'local', 'team'] as const));
  /** The columns without a width (the title and the reason) take what the others leave. */
  const WIDTHS: Partial<Record<(typeof COLUMNS)[number], string>> = { pick: '5rem', local: '9rem', team: '9rem', status: '14rem' };
  /** What a tick on a row says to a screen reader, by the way the copy goes. */
  const rowPickKey = $derived(mode === 'toLocal' ? 'memory.sync.row.pick' : mode === 'toRemote' ? 'memory.sync.row.pickRemote' : 'memory.copy.row.pick');
  const CELL = 'border-b border-line/60 px-3 py-2 align-middle';

  /** Where an entry stands: what the tick would do, or why there is none. A failure says its own reason. */
  function statusOf(row: NotesCopyRow): { text: string; failed: boolean } {
    if (row.failed) {
      return { text: row.failure ?? '', failed: true };
    }

    return { text: t(rowStatusKey(row, mode)), failed: false };
  }

  function columnLabel(column: (typeof COLUMNS)[number]): string {
    if (column === 'title') {
      return t(text.columnTitle);
    }

    if (column === 'status') {
      return t('memory.sync.column.status');
    }

    return column === 'pick' && picking ? t('memory.sync.column.pick') : t(`memory.copy.column.${column}`);
  }

  /** A row opens a group when its repository differs from the row above. */
  function opensGroup(index: number): boolean {
    return index === 0 || rows[index].repo !== rows[index - 1].repo;
  }
</script>

<!-- The entries that differ from the remote copy, or the ones that cannot be copied, a page of the server's plan at a time and grouped by repository. An entry's tick means it is copied, and for a conflict that its version replaces the remote one. A blocked entry has no tick, only its reason, and so has a remote entry that is already here. -->
<div class="flex min-w-0 flex-col overflow-clip rounded-md border border-line" aria-busy={busy}>
  <div class="max-h-[40vh] overflow-auto overscroll-contain">
    <table
      aria-label={t(blocked ? text.blockedTableLabel : picking ? pickText(kind, mode).pickTitle : text.tableLabel)}
      style:min-width={minTableWidth(COLUMNS.map((column) => WIDTHS[column]))}
      class="w-full table-fixed border-separate border-spacing-0 text-sm"
    >
      <thead class="sticky top-0 z-10 bg-surface-1">
        <tr>
          {#each COLUMNS as column (column)}
            <th
              scope="col"
              style:width={WIDTHS[column]}
              class="h-9 border-b border-line px-3 text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase {column === 'title' && blocked ? 'w-2/5' : ''}"
            >
              <span class="truncate">{columnLabel(column)}</span>
            </th>
          {/each}
        </tr>
      </thead>

      <tbody class="transition-opacity duration-[var(--dur-fast)] {busy ? 'opacity-60' : ''}">
        {#each rows as row, index (`${row.localId}:${row.teamId}:${index}`)}
          {#if opensGroup(index)}
            <tr><th scope="rowgroup" colspan={COLUMNS.length} class="h-9 border-b border-line bg-surface-2 px-3 text-left text-xs font-medium text-fg-2" title={row.repo ?? undefined}>{row.scope === 'global' ? t(text.groupGlobal) : noteScopeText(row)}</th></tr>
          {/if}

          <tr class="h-11">
            {#if blocked}
              <td class={CELL}><div class="[overflow-wrap:anywhere] text-fg">{spelledOut(row.title)}</div></td>
              <td class={CELL}><div class="[overflow-wrap:anywhere] text-warning">{row.failure}</div></td>
            {:else}
              {@const id = tickId(row, mode)}
              <!-- Only a row that can be picked has a tick that works: the others keep a greyed one, so the columns line up, and say why in their status. -->
              <td class="{CELL} py-0">
                <Checkbox
                  checked={id !== null && row.pickable && picked(id)}
                  disabled={id === null || !row.pickable}
                  label={t(rowPickKey, { title: spelledOut(row.title) })}
                  onchange={(on) => id !== null && ontoggle(id, on)}
                />
              </td>
              <td class={CELL}><div class="[overflow-wrap:anywhere] text-fg">{spelledOut(row.title)}</div></td>
              {#each [row.localUpdatedAt, row.teamUpdatedAt] as moment, index (index)}
                <td class="{CELL} tabular text-fg-3" title={moment ? formatDateTime(moment) : undefined}>{moment ? relativeTime(moment) : '–'}</td>
              {/each}
              {#if picking}
                {@const status = statusOf(row)}
                <td class={CELL}><div class="[overflow-wrap:anywhere] {status.failed ? 'text-warning' : 'text-fg-3'}">{status.text}</div></td>
              {/if}
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
