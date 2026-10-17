<script lang="ts" generics="T extends { id: number }, V extends string">
  import { type Snippet, tick } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import OptionList from '$lib/components/OptionList.svelte';
  import OptionStrip from '$lib/components/OptionStrip.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import { SIDE_PANEL_WIDTH } from '$lib/components/side-panel';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import type { StripOption } from '$lib/components/strip';
  import type { Column, Sort } from '$lib/components/table';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryScriptsAvailability } from '$lib/ipc/bindings/MemoryScriptsAvailability';
  import type { IpcFailure } from '$lib/ipc/transport';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';

  import MemoryUnavailable from './MemoryUnavailable.svelte';

  interface Props {
    /** What the table is read with: its state and a way to read it again. */
    loader: { status: ViewStatus; error: IpcFailure | null; refresh: (options?: { quiet?: boolean }) => Promise<void> };
    /** Whether a page was read at all yet. */
    loaded: boolean;
    /** Why there is nothing to list; null while the list can be read. */
    availability: MemoryScriptsAvailability | null;
    /** Nothing at all is kept (not only nothing that matches the search). */
    none: boolean;
    emptyIcon: IconComponent;
    emptyTitle: string;
    emptyBody: string;
    failedTitle: string;
    loadingLabel: string;
    /** What a search with no results says; null while there are rows to show. */
    noMatch: string | null;
    /** The kinds, groups or scopes, each with its count: chips above the table, or a column beside it in a wide view. */
    chips: { label: string; idBase: string; options: StripOption<V>[]; value: V; onchange: (value: V) => void };
    /** How much there is in the list ("3 scripts"), said once: beside the chips, or on top of the column. */
    count: string;
    /** What the empty detail column of a wide view says until a row is chosen. */
    detailEmpty: string;
    tableLabel: string;
    /** Names the table for the widths the user gives its columns. */
    tableId: string;
    columns: Column[];
    /** The rows of the page on screen. */
    rows: T[];
    sort: Sort;
    onsort: (sort: Sort) => void;
    page: number;
    pageSize: number;
    /** How many rows there are in all pages. */
    total: number;
    onpage: (page: number) => void;
    /** The row whose panel is open; null for none. */
    openId: number | null;
    /** Opens the panel of a row, or closes it with null. */
    onopen: (id: number | null) => void;
    cell: Snippet<[T, Column]>;
    /** The panel of the open row. It is given the way to close itself, which also puts the focus back on the row, and whether it fills a column (a wide view) or slides over the view's edge. */
    panel: Snippet<[{ close: () => void; inline: boolean }]>;
  }

  let {
    loader,
    loaded,
    availability,
    none,
    emptyIcon: EmptyIcon,
    emptyTitle,
    emptyBody,
    failedTitle,
    loadingLabel,
    noMatch,
    chips,
    count,
    detailEmpty,
    tableLabel,
    tableId,
    columns,
    rows,
    sort,
    onsort,
    page,
    pageSize,
    total,
    onpage,
    openId,
    onopen,
    cell,
    panel,
  }: Props = $props();

  /** The width of the view from which it has room for three columns: the kinds or groups, the table, and the detail, which is always there. */
  const WIDE_FROM_PX = 1360;

  /** The column of the kinds or groups: the one choice with a long label, "Needs review", and its count, fit in it. */
  const SIDEBAR_WIDTH = '15rem';

  let width = $state(0);
  let grid: { focusRow: (key: string | number) => void } | undefined = $state();

  const wide = $derived(width >= WIDE_FROM_PX);

  /** Closes the panel and hands the focus back to the row it was opened from. */
  async function close(): Promise<void> {
    const id = openId;

    onopen(null);

    if (id !== null) {
      await tick();
      grid?.focusRow(id);
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && openId !== null && !event.defaultPrevented) {
      event.preventDefault();
      event.stopPropagation();
      void close();
    }
  }
</script>

<!-- Everything that is bigger than the view is clipped here, not just hidden: a clipped box cannot be scrolled at all, so
     nothing in the view (a focused row, a panel on its way in) can push the shell around it up or down. -->
<div bind:clientWidth={width} class="relative min-h-0 flex-1 overflow-clip">
  {#if loader.status === 'error'}
    <div class="grid size-full place-items-center">
      <EmptyState icon={EmptyIcon} title={failedTitle} body={loader.error?.message}>
        {#snippet action()}
          <Button variant="primary" onclick={() => void loader.refresh()}>{t('memory.tryAgain')}</Button>
        {/snippet}
      </EmptyState>
    </div>
  {:else if !loaded}
    <div class="flex flex-col gap-3 p-8" role="status" aria-label={loadingLabel}>
      <Skeleton width="14rem" height="1.75rem" />
      <Skeleton height="2.5rem" />
      <Skeleton height="2.5rem" />
    </div>
  {:else if availability && availability.kind !== 'available'}
    <MemoryUnavailable {availability} onretry={() => void loader.refresh()} onrefresh={() => loader.refresh({ quiet: true })} />
  {:else if none}
    <div class="grid size-full place-items-center">
      <EmptyState icon={EmptyIcon} title={emptyTitle} body={emptyBody} />
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="absolute inset-0 flex overflow-clip" onkeydown={onKeydown}>
      {#if wide}
        <div class="flex shrink-0 flex-col border-r border-line" style:width={SIDEBAR_WIDTH}>
          <p class="tabular flex h-9 shrink-0 items-center border-b border-line px-5 text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase">{count}</p>

          <div class="min-h-0 flex-1 overflow-y-auto p-3">
            <OptionList options={chips.options} value={chips.value} label={chips.label} idBase={chips.idBase} onchange={chips.onchange} />
          </div>
        </div>
      {/if}

      <!-- Below the wide layout the panel slides over the table: the filters and the table keep their width. -->
      <div class="flex min-h-0 min-w-0 flex-1 flex-col">
        {#if !wide}
          <div class="flex h-14 shrink-0 items-center gap-4 border-b border-line px-4">
            <p class="tabular shrink-0 text-sm font-medium text-fg-2">{count}</p>

            <div class="min-w-0 flex-1 overflow-x-auto">
              <OptionStrip options={chips.options} value={chips.value} kind="radio" appearance="pill" label={chips.label} idBase={chips.idBase} onchange={chips.onchange} class="shrink-0" />
            </div>
          </div>
        {/if}

        {#if noMatch}
          <p class="min-h-0 flex-1 px-4 py-10 text-center text-sm text-fg-3" role="status">{noMatch}</p>
        {:else}
          <DataTable
            bind:this={grid}
            label={tableLabel}
            {tableId}
            {columns}
            {rows}
            rowKey={(row) => row.id}
            selectedKey={openId}
            {sort}
            {onsort}
            onopen={(row) => onopen(row.id)}
            onmove={(row) => openId !== null && onopen(row.id)}
            busy={loader.status === 'loading'}
            {cell}
          />

          <Pagination {page} {pageSize} {total} {onpage} />
        {/if}
      </div>

      {#if wide}
        <!-- The detail has its own column, always there: what is chosen swaps in with a short fade, with nothing sliding. -->
        <div class="relative flex shrink-0 flex-col border-l border-line-strong bg-surface-1" style:width={SIDE_PANEL_WIDTH}>
          {#if openId === null}
            <div in:rise={{ y: 6, duration: 240 }} class="grid min-h-0 flex-1 place-items-center p-8">
              <div class="flex max-w-60 flex-col items-center gap-3 text-center">
                <EmptyIcon size={ICON.lg} strokeWidth={1.6} class="text-fg-3" aria-hidden="true" />
                <p class="text-sm text-fg-3">{detailEmpty}</p>
              </div>
            </div>
          {:else}
            {#key openId}
              <div in:rise={{ y: 6, duration: 240 }} class="flex min-h-0 flex-1 flex-col">
                {@render panel({ close: () => void close(), inline: true })}
              </div>
            {/key}
          {/if}
        </div>
      {:else}
        {@render panel({ close: () => void close(), inline: false })}
      {/if}
    </div>
  {/if}
</div>
