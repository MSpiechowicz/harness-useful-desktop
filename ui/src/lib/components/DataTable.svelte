<script lang="ts" generics="T">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { type Snippet, untrack } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import ColumnHandle from './ColumnHandle.svelte';
  import { ICON } from './icon';
  import { type Column, type ColumnWidths, MIN_COLUMN_PX, nextSort, readColumnWidths, saveColumnWidths, type Sort } from './table';

  interface Props {
    /** Accessible name of the table. */
    label: string;
    /** Names the table among the others, so the widths the user gives its columns are kept under it. */
    tableId: string;
    columns: Column[];
    rows: T[];
    rowKey: (row: T) => string | number;
    /** The row whose detail is open. */
    selectedKey: string | number | null;
    sort: Sort;
    /** A header of a sortable column was clicked: the sort to apply. */
    onsort: (sort: Sort) => void;
    /** A row was clicked, or Enter or Space was pressed on it. */
    onopen: (row: T) => void;
    /** Arrow keys moved to another row; the focus has moved already. */
    onmove?: (row: T) => void;
    /** Reading the next page: the rows stay, dimmed. */
    busy?: boolean;
    cell: Snippet<[T, Column]>;
  }

  let { label, tableId, columns, rows, rowKey, selectedKey, sort, onsort, onopen, onmove, busy = false, cell }: Props = $props();

  const ROW_PX = 44;

  /** What the column without a width of its own, the one that takes what the others leave, is never squeezed below. */
  const MIN_FLEX_PX = 160;
  const FROM = { sm: 'hidden @lg:table-cell', md: 'hidden @2xl:table-cell', lg: 'hidden @5xl:table-cell' } as const;

  let viewport: HTMLElement | undefined = $state();
  let head: HTMLElement | undefined = $state();
  let body: HTMLElement | undefined = $state();
  let cells: Record<string, HTMLElement | undefined> = $state({});
  let widths: ColumnWidths = $state(untrack(() => readColumnWidths(tableId, columns)));

  // The column without a width stays the flexible one. Columns after it hang on the right of the table, so growing one takes room from the flexible one on its left edge.
  const flexKey = $derived(columns.find((column) => !column.width)?.key);
  const flexIndex = $derived(columns.findIndex((column) => column.key === flexKey));

  // One row is a tab stop: the open one, or else the first. Arrow keys move between the rows from there.
  const stopKey = $derived(rows.some((row) => rowKey(row) === selectedKey) ? selectedKey : rows[0] ? rowKey(rows[0]) : null);

  function visible(column: Column): string {
    return column.from ? FROM[column.from] : '';
  }

  function widthOf(column: Column): number {
    return cells[column.key]?.offsetWidth ?? 0;
  }

  /** Gives a column a width, held between the narrowest allowed and what the flexible column can spare; answers the width it got. */
  function resize(column: Column, px: number): number {
    const spare = flexKey ? Math.max(0, (cells[flexKey]?.offsetWidth ?? 0) - MIN_FLEX_PX) : Number.POSITIVE_INFINITY;
    const width = Math.round(Math.min(Math.max(px, MIN_COLUMN_PX), Math.max(MIN_COLUMN_PX, widthOf(column) + spare)));

    widths[column.key] = width;

    return width;
  }

  function resetWidth(column: Column): void {
    delete widths[column.key];
    saveColumnWidths(tableId, widths);
  }

  function rowElement(key: string | number): HTMLElement | null {
    return body?.querySelector<HTMLElement>(`[data-row="${CSS.escape(String(key))}"]`) ?? null;
  }

  /**
   * Scrolls the table's own viewport just far enough to show a row below the sticky header. Nothing outside the table
   * moves: `scrollIntoView` would also scroll every clipping ancestor up to the page, and the app's shell would slide.
   */
  function reveal(row: HTMLElement): void {
    if (!viewport) {
      return;
    }

    const view = viewport.getBoundingClientRect();
    const box = row.getBoundingClientRect();
    const top = view.top + (head?.offsetHeight ?? 0);

    if (box.top < top) {
      viewport.scrollTop += box.top - top;
    } else if (box.bottom > view.bottom) {
      viewport.scrollTop += box.bottom - view.bottom;
    }
  }

  /** Puts the focus on a row, e.g. back on the one whose panel was just closed. */
  export function focusRow(key: string | number): void {
    const row = rowElement(key);

    if (row) {
      row.focus({ preventScroll: true });
      reveal(row);
    }
  }

  function onKeydown(event: KeyboardEvent, row: T, index: number): void {
    if (event.target !== event.currentTarget) {
      return;
    }

    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onopen(row);

      return;
    }

    const targets: Record<string, number> = { ArrowDown: index + 1, ArrowUp: index - 1, Home: 0, End: rows.length - 1 };
    const target = rows[targets[event.key]];

    if (!(event.key in targets) || !target) {
      return;
    }

    event.preventDefault();
    focusRow(rowKey(target));
    onmove?.(target);
  }
</script>

{#snippet arrow(active: boolean)}
  {#if active}
    <span class="grid size-4 shrink-0 place-items-center" aria-hidden="true">
      {#if sort.desc}<ArrowDown size={ICON.sm} />{:else}<ArrowUp size={ICON.sm} />{/if}
    </span>
  {/if}
{/snippet}

<!-- A grid of the page on screen: every row is one stop of the Tab key, the arrow keys move between rows, and a row opens
     its detail. Headers of sortable columns are buttons. The container query decides which columns fit. It lies in a
     flex column and is its own scrolling viewport, with the header held at the top. A chevron ends every row: it is what
     says that a row opens something. -->
<div bind:this={viewport} class="@container min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain [scrollbar-gutter:stable]">
  <table role="grid" aria-label={label} aria-busy={busy} class={cx('w-full table-fixed border-separate border-spacing-0 text-sm transition-opacity duration-[var(--dur-base)]', busy && 'opacity-60')}>
    <thead bind:this={head} class="group/head sticky top-0 z-10 bg-surface-1">
      <tr>
        {#each columns as column, index (column.key)}
          {@const active = sort.key === column.key}

          <th
            bind:this={cells[column.key]}
            role="columnheader"
            scope="col"
            aria-label={column.label}
            aria-sort={column.sortable ? (active ? (sort.desc ? 'descending' : 'ascending') : 'none') : undefined}
            style:width={widths[column.key] ? `${widths[column.key]}px` : column.width}
            class={cx('relative h-9 select-none border-b border-line px-3 text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase', column.align === 'end' ? 'text-right' : 'text-left', visible(column))}
          >
            {#if column.sortable}
              <button
                type="button"
                class={cx(
                  '-mx-2 inline-flex h-7 max-w-full items-center gap-1 rounded-sm px-2 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2',
                  active && 'text-fg',
                )}
                onclick={() => onsort(nextSort(sort, column))}
              >
                <!-- The arrow goes on the side away from the edge the text is set to, so the label stays where it is when a sort turns on. -->
                {#if column.align === 'end'}{@render arrow(active)}{/if}
                <span class="truncate">{column.label}</span>
                {#if column.align !== 'end'}{@render arrow(active)}{/if}
              </button>
            {:else}
              <span class="truncate">{column.label}</span>
            {/if}

            {#if column.key !== flexKey}
              <ColumnHandle
                column={column.label}
                edge={flexIndex >= 0 && index > flexIndex ? 'start' : 'end'}
                measure={() => widthOf(column)}
                onresize={(px) => resize(column, px)}
                oncommit={() => saveColumnWidths(tableId, widths)}
                onreset={() => resetWidth(column)}
              />
            {/if}
          </th>
        {/each}
        <th aria-hidden="true" class="h-9 w-10 border-b border-line"></th>
      </tr>
    </thead>

    <tbody bind:this={body}>
      {#each rows as row, index (rowKey(row))}
        {@const key = rowKey(row)}
        {@const current = key === selectedKey}

        <tr
          data-row={key}
          tabindex={key === stopKey ? 0 : -1}
          aria-selected={current}
          style:height="{ROW_PX}px"
          class={cx(
            'group/row cursor-pointer transition-colors duration-[var(--dur-fast)] outline-offset-[-2px] focus-visible:outline-2',
            current ? 'bg-accent-wash' : 'hover:bg-hover active:bg-press',
          )}
          onclick={() => onopen(row)}
          onkeydown={(event) => onKeydown(event, row, index)}
        >
          {#each columns as column (column.key)}
            <td role="gridcell" class={cx('border-b border-line/60 px-3', column.align === 'end' ? 'text-right' : 'text-left', visible(column))}>
              <div class="truncate">{@render cell(row, column)}</div>
            </td>
          {/each}
          <td aria-hidden="true" class="border-b border-line/60 pr-3 text-right">
            <ChevronRight
              size={ICON.sm}
              class={cx('inline-block transition-colors duration-[var(--dur-fast)]', current ? 'text-accent-text' : 'text-fg-3 group-hover/row:text-fg-2 group-focus-visible/row:text-fg-2')}
            />
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
