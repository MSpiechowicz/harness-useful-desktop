<script lang="ts">
  import { lineSign, lineSpoken, type IndexedLine, type PairRow } from '$lib/git/diff-rows';
  import { commentOnLineClick, type PickHandlers } from '$lib/git/comment-range';
  import type { CommentSide } from '$lib/ipc/bindings/CommentSide';
  import type { HighlightedLine } from '$lib/markdown/highlight';
  import { cx } from '$lib/utils/cx';

  import { DIFF_TEXT } from './diff-tones';
  import DiffViewAddComment from './DiffViewAddComment.svelte';
  import DiffViewCode from './DiffViewCode.svelte';
  import DiffViewGutter from './DiffViewGutter.svelte';

  interface Props {
    row: PairRow;
    highlighted: ReadonlyMap<number, HighlightedLine>;
    /** Flat indexes inside the range being commented on. */
    covered: ReadonlySet<number>;
    pick: PickHandlers;
    /** The normalized search query, and the flat index of the match being stepped through. */
    query?: string | null;
    currentMatch?: number;
  }

  let { row, highlighted, covered, pick, query = null, currentMatch = -1 }: Props = $props();

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
    {@const cell = half.cell}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -- the gutter buttons are the keyboard path; clicking the line is a pointer shortcut -->
    <div
      class={cx('group/line flex min-w-0 items-start', cell && 'cursor-pointer', tone(half.cell, half.side))}
      data-side={half.side}
      data-kind={half.cell?.line.kind ?? 'empty'}
      onclick={(event) => cell && commentOnLineClick(event, pick, cell.index, half.side)}
    >
      {#if half.cell}
        <DiffViewGutter number={half.side === 'old' ? half.cell.line.oldNo : half.cell.line.newNo} side={half.side} index={half.cell.index} {pick} />
        <DiffViewAddComment sign={lineSign(half.cell.line.kind)} tone={DIFF_TEXT[half.cell.line.kind]} index={half.cell.index} side={half.side} {pick} />
        <span class="sr-only">{lineSpoken(half.cell.line.kind)}</span>
        <DiffViewCode text={half.cell.line.text} tokens={highlighted.get(half.cell.index)} kind={half.cell.line.kind} {query} current={half.cell.index === currentMatch} />
      {:else}
        <span class="min-h-[20px] w-full" aria-hidden="true"></span>
      {/if}
    </div>
  {/each}
</div>
