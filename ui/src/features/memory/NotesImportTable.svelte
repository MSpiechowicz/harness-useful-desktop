<script lang="ts">
  import Checkbox from '$lib/components/Checkbox.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import type { Column } from '$lib/components/table';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { NotesImportRow } from '$lib/ipc/bindings/NotesImportRow';
  import { cx } from '$lib/utils/cx';

  import NoteKindBadge from './NoteKindBadge.svelte';
  import { IMPORT_PAGE_SIZE, importable, pageOfRows, problemText, reviewColumns } from './notes-import';
  import { spelledOut } from './script-format';

  interface Props {
    rows: readonly NotesImportRow[];
    picks: ReadonlySet<number>;
    ontoggle: (index: number, on: boolean) => void;
  }

  let { rows, picks, ontoggle }: Props = $props();

  // Columns that do not fit the dialog are left out, as the Notes table does: the container decides.
  const FROM: Record<NonNullable<Column['from']>, string> = { sm: 'hidden @lg:table-cell', md: 'hidden @2xl:table-cell', lg: 'hidden @4xl:table-cell' };

  let page = $state(0);

  const shown = $derived(pageOfRows(rows, page));
  const columns = $derived(reviewColumns());

  function label(row: NotesImportRow): string {
    return row.title === '' ? t('memory.import.row.pickUntitled', { row: row.index + 1 }) : t('memory.import.row.pick', { title: spelledOut(row.title) });
  }

  function scopeText(row: NotesImportRow): string {
    if (row.scope === null) {
      return t('memory.import.scope.default');
    }

    return t(row.scope === 'repo' ? 'memory.editor.scope.repo' : 'memory.editor.scope.global');
  }
</script>

<!-- Every row of the file with its checkbox. A row with a problem cannot be ticked and says what is wrong, a row that repeats the title of
     an earlier one starts unticked. A file of many notes is paged here, in the dialog, as the server has nothing to page. -->
<div class="@container flex min-w-0 flex-col overflow-clip rounded-md border border-line">
  <div class="max-h-[50vh] overflow-y-auto overscroll-contain">
    <table aria-label={t('memory.import.table.label')} class="w-full table-fixed border-separate border-spacing-0 text-sm">
      <thead class="sticky top-0 z-10 bg-surface-1">
        <tr>
          {#each columns as column (column.key)}
            <th
              scope="col"
              style:width={column.width}
              class={cx('h-9 border-b border-line px-3 text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase', column.from && FROM[column.from])}
            >
              <span class="truncate">{column.label}</span>
            </th>
          {/each}
        </tr>
      </thead>

      <tbody>
        {#each shown as row (row.index)}
          <tr class="h-11 align-top">
            <td class="border-b border-line/60 px-3 py-0 align-middle">
              <Checkbox checked={picks.has(row.index)} label={label(row)} disabled={!importable(row)} onchange={(on) => ontoggle(row.index, on)} />
            </td>
            <td class="tabular border-b border-line/60 px-3 py-2 align-middle text-fg-3">{row.index + 1}</td>
            <td class="border-b border-line/60 px-3 py-2 align-middle">
              <div class="[overflow-wrap:anywhere] text-fg">{row.title === '' ? t('memory.import.untitled') : spelledOut(row.title)}</div>

              {#each row.problems as problem, index (index)}
                <p class="text-xs text-danger">{problemText(problem)}</p>
              {/each}

              {#if row.duplicateOf !== null}
                <p class="text-xs text-warning">{t('memory.import.duplicate', { row: row.duplicateOf + 1 })}</p>
              {/if}
            </td>
            <td class="border-b border-line/60 px-3 py-2 align-middle"><NoteKindBadge kind={row.kind} size="lg" /></td>
            <td class={cx('border-b border-line/60 px-3 py-2 align-middle text-fg-2', FROM.md)}><div class="truncate">{row.tags.join(', ')}</div></td>
            <td class="border-b border-line/60 px-3 py-2 align-middle text-fg-2"><div class="truncate">{scopeText(row)}</div></td>
            <td class={cx('border-b border-line/60 px-3 py-2 align-middle text-fg-3', FROM.lg)}><div class="truncate" title={row.bodyPreview}>{row.bodyPreview}</div></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if rows.length > IMPORT_PAGE_SIZE}
    <Pagination {page} pageSize={IMPORT_PAGE_SIZE} total={rows.length} onpage={(next) => (page = next)} />
  {/if}
</div>
