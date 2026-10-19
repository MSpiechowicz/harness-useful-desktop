<script lang="ts" generics="V, G extends { id: string; views: V[] }">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import { ICON } from '../icon';
  import SortButton from '../SortButton.svelte';
  import { ariaSort, nextSort, type Sort } from '../table';
  import type { DocColumn } from './doc-groups';

  interface Props {
    /** Accessible name of the table. */
    label: string;
    columns: DocColumn[];
    /** The sort the rows of every group are in; null for the run order. Only the columns with a key sort. */
    sort?: Sort | null;
    /** A header of a sortable column was clicked: the sort to apply. */
    onsort?: (sort: Sort) => void;
    groups: G[];
    rowKey: (view: V) => string;
    /** The row whose panel is open. */
    openId: string | null;
    onopen: (id: string) => void;
    /** Moves the open panel along when the arrow keys move the focus. */
    onmove: (id: string) => void;
    /** The heading row of a group (a `DocGroupHeader`). */
    heading: Snippet<[G]>;
    /** More rows right after a group's heading, e.g. why the team's documents could not be read. */
    note?: Snippet<[G]>;
    /** The cells of one row, without the chevron that ends it. */
    cells: Snippet<[V]>;
    /** What an empty group says, in its one row. */
    empty: Snippet<[G]>;
  }

  let { label, columns, sort = null, onsort, groups, rowKey, openId, onopen, onmove, heading, note, cells, empty }: Props = $props();

  let body: HTMLElement | undefined = $state();

  const span = $derived(columns.length + 1);
  const first = $derived(groups.flatMap((group) => group.views).map(rowKey)[0] ?? null);
  const stop = $derived(groups.some((group) => group.views.some((view) => rowKey(view) === openId)) ? openId : first);

  function rows(): HTMLElement[] {
    return [...(body?.querySelectorAll<HTMLElement>('[data-row]') ?? [])];
  }

  /** Puts the focus on a row, e.g. back on the one whose panel was just closed. */
  export function focusRow(id: string): void {
    body?.querySelector<HTMLElement>(`[data-row="${CSS.escape(id)}"]`)?.focus({ preventScroll: false });
  }

  function onKeydown(event: KeyboardEvent, id: string): void {
    if (event.target !== event.currentTarget) {
      return;
    }

    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onopen(id);

      return;
    }

    const all = rows();
    const at = all.findIndex((row) => row.dataset.row === id);
    const targets: Record<string, number> = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: all.length - 1 };
    const target = all[targets[event.key]];

    if (!target) {
      return;
    }

    event.preventDefault();
    target.focus();
    onmove(target.dataset.row ?? id);
  }
</script>

<!-- A table of the documents in their groups: every row is one stop of the Tab key, the arrow keys move between rows, and a row opens its panel.
     The controls of a row stay out of the way of that: they work the document without opening it. A chevron ends every row, as it does in the Memory tables. -->
<div class="doc-table @container min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain [scrollbar-gutter:stable]">
  <table role="grid" aria-label={label} class="w-full table-fixed border-separate border-spacing-0 text-sm">
    <colgroup>
      {#each columns as column (column.label)}
        <col class={column.col} />
      {/each}
      <col class="w-10" />
    </colgroup>

    <thead class="sticky top-0 z-10 bg-surface-1">
      <tr class="h-9 text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase">
        {#each columns as column (column.label)}
          {@const key = column.key}

          <th scope="col" aria-sort={key && onsort ? ariaSort(sort, key) : undefined} class={cx('border-b border-line px-3 font-medium', column.right ? 'text-right' : 'text-left', column.head)}>
            {#if key && onsort}
              <SortButton label={column.label} active={sort?.key === key} desc={sort?.desc ?? false} end={column.right} onclick={() => onsort(nextSort(sort, { key, descFirst: column.descFirst }))} />
            {:else}
              {column.label}
            {/if}
          </th>
        {/each}
        <th scope="col" aria-hidden="true" class="border-b border-line"></th>
      </tr>
    </thead>

    <tbody bind:this={body}>
      {#each groups as group (group.id)}
        {@render heading(group)}
        {@render note?.(group)}

        {#each group.views as view, index (rowKey(view))}
          {@const key = rowKey(view)}
          {@const current = key === openId}

          <tr
            data-row={key}
            tabindex={key === stop ? 0 : -1}
            aria-selected={current}
            style:height="44px"
            style:--i={index}
            class={cx(
              'group/row cursor-pointer outline-offset-[-2px] transition-colors duration-[var(--dur-fast)] focus-visible:outline-2',
              current ? 'bg-accent-wash' : 'hover:bg-hover active:bg-press',
            )}
            onclick={() => onopen(key)}
            onkeydown={(event) => onKeydown(event, key)}
          >
            {@render cells(view)}

            <td aria-hidden="true" class="border-b border-line/60 pr-3 text-right">
              <ChevronRight size={ICON.sm} class={cx('inline-block transition-colors duration-[var(--dur-fast)]', current ? 'text-accent-text' : 'text-fg-3 group-hover/row:text-fg-2')} />
            </td>
          </tr>
        {/each}

        {#if group.views.length === 0}
          <tr>
            <td colspan={span} class="h-11 border-b border-line/60 px-3 text-sm text-fg-3">{@render empty(group)}</td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
</div>

<style>
  /* Rows come in one after the other, a short rise each. Only the contents of the cells move, as a row itself cannot be transformed everywhere. */
  @keyframes -global-doc-row-in {
    from {
      opacity: 0;
      transform: translateY(var(--shift));
    }
  }

  .doc-table :global(tr[data-row] > td > *) {
    animation: doc-row-in var(--dur-enter) var(--ease-out-expo) both;
    animation-delay: calc(min(var(--i, 0), 10) * 28ms * var(--stagger));
  }
</style>
