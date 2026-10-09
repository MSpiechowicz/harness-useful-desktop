<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  import { flattenTree, type TreeRow } from './config-tree';
  import ConfigValueView from './ConfigValueView.svelte';

  interface Props {
    fields: ConfigField[];
    /** Accessible name of the tree. */
    label: string;
    /** Case-insensitive filter over keys and plain values (never secrets). */
    query?: string;
    /** Branches shallower than this start expanded. */
    defaultDepth?: number;
  }

  let { fields, label, query = '', defaultDepth = 1 }: Props = $props();

  const INDENT_PX = 18;

  let overrides = $state.raw(new Map<string, boolean>());
  let focusIndex = $state(0);
  let rowElements: (HTMLElement | undefined)[] = $state([]);

  const rows = $derived(flattenTree(fields, { query, overrides, defaultDepth }));
  const tabStop = $derived(Math.min(focusIndex, Math.max(rows.length - 1, 0)));

  function setOpen(row: TreeRow, open: boolean): void {
    overrides = new Map(overrides).set(row.id, open);
  }

  function toggleable(row: TreeRow): boolean {
    return row.branch || row.longText;
  }

  function focusRow(index: number): void {
    focusIndex = index;
    rowElements[index]?.focus();
  }

  function onKeydown(event: KeyboardEvent): void {
    const row = rows[tabStop];
    if (!row) {
      return;
    }

    const vertical = rovingIndex(event.key, tabStop, rows.length, 'vertical');
    if (vertical !== null) {
      event.preventDefault();
      focusRow(vertical);

      return;
    }

    if (event.key === 'ArrowRight') {
      event.preventDefault();

      if (toggleable(row) && !row.expanded) {
        setOpen(row, true);
      } else if (row.branch && row.expanded) {
        focusRow(tabStop + 1);
      }
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();

      if (toggleable(row) && row.expanded) {
        setOpen(row, false);
      } else if (row.parentId !== null) {
        focusRow(rows.findIndex((candidate) => candidate.id === row.parentId));
      }
    } else if ((event.key === 'Enter' || event.key === ' ') && toggleable(row)) {
      event.preventDefault();
      setOpen(row, !row.expanded);
    }
  }

  function activate(row: TreeRow, index: number): void {
    focusIndex = index;

    if (toggleable(row)) {
      setOpen(row, !row.expanded);
    }
  }
</script>

{#if rows.length === 0}
  <p class="px-3 py-6 text-center text-sm text-fg-3" role="status">
    {query.trim() === '' ? 'No entries in this file.' : 'No entries match your filter.'}
  </p>
{:else}
  <!-- Flat rows with aria-level: a valid ARIA tree whose keyboard model is a single roving tab stop. -->
  <div role="tree" aria-label={label} tabindex="-1" class="flex flex-col font-mono text-sm" onkeydown={onKeydown}>
    {#each rows as row, index (row.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -- keys are handled once for the whole tree -->
      <div
        bind:this={rowElements[index]}
        role="treeitem"
        aria-selected={index === tabStop}
        aria-level={row.depth + 1}
        aria-posinset={row.posInSet}
        aria-setsize={row.setSize}
        aria-expanded={toggleable(row) ? row.expanded : undefined}
        tabindex={index === tabStop ? 0 : -1}
        onclick={() => activate(row, index)}
        onfocus={() => (focusIndex = index)}
        style:padding-left="{row.depth * INDENT_PX + 8}px"
        class={cx(
          'group flex items-start gap-2 rounded-sm py-1 pr-3 outline-offset-[-2px] transition-colors duration-[var(--dur-fast)] hover:bg-hover',
          toggleable(row) && 'cursor-pointer',
        )}
      >
        <span class="mt-0.5 grid size-4 shrink-0 place-items-center text-fg-3" aria-hidden="true">
          {#if toggleable(row)}
            <ChevronRight size={14} class={cx('transition-transform duration-[var(--dur-fast)]', row.expanded && 'rotate-90')} />
          {/if}
        </span>

        <span class="shrink-0 text-fg-2">{row.key}</span>

        <span class="flex min-w-0 flex-1 items-baseline">
          <ConfigValueView value={row.value} expanded={row.longText && row.expanded} class="min-w-0" />
        </span>
      </div>
    {/each}
  </div>
{/if}
