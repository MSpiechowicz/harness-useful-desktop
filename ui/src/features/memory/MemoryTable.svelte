<script lang="ts" generics="T extends { id: number | string }">
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import DocSplit from '$lib/components/docs/DocSplit.svelte';
  import DocStage from '$lib/components/docs/DocStage.svelte';
  import { closePanel } from '$lib/components/docs/doc-view';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import FilterRow from '$lib/components/FilterRow.svelte';
  import LoadingSpinner from '$lib/components/LoadingSpinner.svelte';
  import Pagination from '$lib/components/Pagination.svelte';
  import type { Column, Sort } from '$lib/components/table';
  import { LOADING_DELAY_MS } from '$lib/design/motion';
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
    /** What the empty list offers to do, e.g. add the first item. */
    emptyAction?: Snippet;
    failedTitle: string;
    loadingLabel: string;
    /** What a search with no results says; null while there are rows to show. */
    noMatch: string | null;
    /** How much there is in the list ("3 scripts"), said once, in the row of the filters above the table. */
    count?: string;
    /** The filter menus of that row. */
    filters: Snippet;
    /** Something quiet at the far end of that row, e.g. the hint to update Harness Memory. */
    hint?: Snippet;
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
    openId: T['id'] | null;
    /** Opens the panel of a row, or closes it with null. */
    onopen: (id: T['id'] | null) => void;
    cell: Snippet<[T, Column]>;
    /** The panel of the open row. It is given the way to close itself, which also puts the focus back on the row. It slides over the view's right edge. */
    panel: Snippet<[{ close: () => void }]>;
  }

  let {
    loader,
    loaded,
    availability,
    none,
    emptyIcon: EmptyIcon,
    emptyTitle,
    emptyBody,
    emptyAction,
    failedTitle,
    loadingLabel,
    noMatch,
    count,
    filters,
    hint,
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

  let grid: { focusRow: (key: string | number) => void } | undefined = $state();

  /**
   * A read for a changed search or filter is under way. The page still on screen answers the old one, so when it is empty, "nothing found"
   * would be a stale claim. The spinner waits LOADING_DELAY_MS first: a quick answer swaps in without the table blinking.
   */
  const reading = $derived(loader.status === 'loading' && rows.length === 0);
  let slow = $state(false);
  const stale = $derived(reading && slow);

  $effect(() => {
    if (!reading) {
      slow = false;

      return;
    }

    const timer = setTimeout(() => (slow = true), LOADING_DELAY_MS);

    return () => clearTimeout(timer);
  });

  /** Closes the panel and hands the focus back to the row it was opened from. */
  function close(): Promise<void> {
    return closePanel(openId === null ? null : String(openId), () => onopen(null), (id) => grid?.focusRow(id));
  }
</script>

{#snippet column()}
  {@render panel({ close: () => void close() })}
{/snippet}

<DocStage>
  {#if loader.status === 'error'}
    <div class="grid size-full place-items-center">
      <EmptyState icon={EmptyIcon} title={failedTitle} body={loader.error?.message}>
        {#snippet action()}
          <Button variant="primary" onclick={() => void loader.refresh()}>{t('memory.tryAgain')}</Button>
        {/snippet}
      </EmptyState>
    </div>
  {:else if !loaded}
    <LoadingSpinner label={loadingLabel} />
  {:else if availability && availability.kind !== 'available'}
    <MemoryUnavailable {availability} onretry={() => void loader.refresh()} onrefresh={() => loader.refresh({ quiet: true })} />
  {:else if none && !stale}
    <div class="grid size-full place-items-center">
      <EmptyState icon={EmptyIcon} title={emptyTitle} body={emptyBody} action={emptyAction} />
    </div>
  {:else}
    <DocSplit openId={openId === null ? null : String(openId)} onclose={() => void close()} panel={column}>
      {#snippet body()}
        <FilterRow {count} trailing={hint}>{@render filters()}</FilterRow>

        {#if stale}
          <LoadingSpinner label={loadingLabel} delay={0} class="flex-1" />
        {:else if noMatch}
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
      {/snippet}
    </DocSplit>
  {/if}
</DocStage>
