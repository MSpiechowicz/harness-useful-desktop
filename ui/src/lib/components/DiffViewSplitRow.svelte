<script lang="ts">
  import { lineSign, lineSpoken, type IndexedLine, type PairRow } from '$lib/git/diff-rows';
  import type { PickHandlers } from '$lib/git/comment-range';
  import type { CommentSide } from '$lib/ipc/bindings/CommentSide';
  import type { HighlightedLine } from '$lib/markdown/highlight';
  import { cx } from '$lib/utils/cx';

  import DiffViewCode from './DiffViewCode.svelte';
  import DiffViewGutter from './DiffViewGutter.svelte';

  interface Props {
    row: PairRow;
    highlighted: ReadonlyMap<number, HighlightedLine>;
    /** Flat indexes inside the range being commented on. */
    covered: ReadonlySet<number>;
    pick: PickHandlers;
  }

  let { row, highlighted, covered, pick }: Props = $props();

  function tone(cell: IndexedLine | null, side: CommentSide): string {
    if (cell === null) {
      return 'bg-surface-2/60';
    }
    if (covered.has(cell.index)) {
      return 'bg-accent-wash';
    }

    // Context sits on both sides without a tint.
    if (cell.line.kind === 'context') {
      return '';
    }

    return side === 'old' ? 'bg-danger-wash' : 'bg-success-wash';
  }
</script>

<div class="grid grid-cols-2 divide-x divide-line" data-pair={row.key}>
  {#each [{ cell: row.left, side: 'old' }, { cell: row.right, side: 'new' }] as const as half (half.side)}
    <div class={cx('flex min-w-0 items-start', tone(half.cell, half.side))} data-side={half.side} data-kind={half.cell?.line.kind ?? 'empty'}>
      {#if half.cell}
        <DiffViewGutter number={half.side === 'old' ? half.cell.line.oldNo : half.cell.line.newNo} side={half.side} index={half.cell.index} {pick} />
        <span class="w-4 shrink-0 text-center font-mono text-[13px] leading-[18px] text-fg-3 select-none" aria-hidden="true">{lineSign(half.cell.line.kind)}</span>
        <span class="sr-only">{lineSpoken(half.cell.line.kind)}</span>
        <DiffViewCode text={half.cell.line.text} tokens={highlighted.get(half.cell.index)} />
      {:else}
        <span class="min-h-[18px] w-full" aria-hidden="true"></span>
      {/if}
    </div>
  {/each}
</div>
