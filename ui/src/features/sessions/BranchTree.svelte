<script lang="ts">
  import GitFork from '@lucide/svelte/icons/git-fork';
  import Tag from '@lucide/svelte/icons/tag';
  import { tick } from 'svelte';

  import VirtualList from '$lib/components/VirtualList.svelte';
  import type { SessionTree } from '$lib/ipc/bindings/SessionTree';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import { kindLabel, nodeTooltip, previewText } from './session-format';
  import TreeGraph from './TreeGraph.svelte';
  import { isFoldKey, layoutTree, type GraphRow } from './tree-layout';

  interface Props {
    tree: SessionTree;
    /** The selected node id. */
    selected: string | null;
    onselect: (id: string) => void;
  }

  let { tree, selected, onselect }: Props = $props();

  const ROW_PX = 28;
  const idBase = uid('tree-gradient');

  let expanded = $state.raw<ReadonlySet<string>>(new Set());
  let list: VirtualList<GraphRow> | undefined = $state();
  let container: HTMLElement | undefined = $state();

  const layout = $derived(layoutTree(tree.nodes, { expanded, keep: new Set(selected ? [selected] : []) }));
  const selectedIndex = $derived(layout.rows.findIndex((row) => row.key === selected));

  function activate(row: GraphRow): void {
    if (isFoldKey(row.key)) {
      expanded = new Set([...expanded, row.key]);

      return;
    }

    onselect(row.key);
  }

  /** Keeps the keyboard-selected row on screen and focused, even when virtualization has not rendered it yet. */
  async function reveal(key: string, index: number): Promise<void> {
    const find = (): HTMLElement | null => container?.querySelector<HTMLElement>(`[data-row-key="${CSS.escape(key)}"]`) ?? null;

    if (!find()) {
      list?.scrollToIndex(index);
      await tick();
    }

    const button = find();
    button?.scrollIntoView?.({ block: 'nearest' });
    button?.focus({ preventScroll: true });
  }

  async function onKeydown(event: KeyboardEvent): Promise<void> {
    const steps: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    const rows = layout.rows;
    let target = -1;

    if (event.key in steps) {
      target = Math.min(rows.length - 1, Math.max(0, (selectedIndex === -1 ? 0 : selectedIndex) + steps[event.key]));
    } else if (event.key === 'Home') {
      target = 0;
    } else if (event.key === 'End') {
      target = rows.length - 1;
    } else {
      return;
    }

    event.preventDefault();
    const row = rows[target];
    if (row && !isFoldKey(row.key)) {
      onselect(row.key);
    }

    if (row) {
      await reveal(row.key, target);
    }
  }
</script>

<!-- Arrow keys move through the rows; the list itself is a plain scroller. -->
<div bind:this={container} class="size-full" role="presentation" onkeydown={onKeydown}>
  <VirtualList bind:this={list} items={layout.rows} estimate={ROW_PX} overscan={12} label="Session branches" key={(row) => row.key} class="size-full">
    {#snippet row(item, index)}
      {@const node = item.node}
      {@const isSelected = item.key === selected}
      <button
        type="button"
        data-row-key={item.key}
        data-active-path={item.active}
        aria-current={isSelected ? 'true' : undefined}
        tabindex={isSelected || (selected === null && index === 0) ? 0 : -1}
        title={node ? nodeTooltip(node) : `Show ${item.folded.length} more messages`}
        class={cx('flex w-full items-center gap-1 pr-3 text-left transition-colors duration-[var(--dur-fast)]', isSelected ? 'bg-accent-wash' : 'hover:bg-hover')}
        style:height="{ROW_PX}px"
        onclick={() => activate(item)}
      >
        <TreeGraph row={item} position={index} lanes={layout.lanes} selected={isSelected} {idBase} />

        {#if node}
          <span class={cx('min-w-0 flex-1 truncate text-sm', item.active ? 'text-fg' : 'text-fg-3')}>
            <span class="mr-1.5 text-xs font-medium text-fg-3">{kindLabel(node.kind)}</span>{previewText(node)}
          </span>

          {#if node.label}
            <span class="inline-flex shrink-0 items-center gap-1 rounded-full bg-info-wash px-2 text-xs text-info"><Tag size={11} aria-hidden="true" />{node.label}</span>
          {/if}
          {#if item.branchPoint}
            <span class="inline-flex shrink-0 items-center gap-1 text-xs text-accent-text" title="{node.childCount} branches start here"><GitFork size={12} aria-hidden="true" />{node.childCount}<span class="sr-only">{' branches'}</span></span>
          {/if}
        {:else}
          <span class="min-w-0 flex-1 truncate text-sm text-fg-3 italic">{item.folded.length} more messages</span>
        {/if}
      </button>
    {/snippet}
  </VirtualList>
</div>
