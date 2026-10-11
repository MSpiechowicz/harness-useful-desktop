<script lang="ts">
  import { lineSign, lineSpoken, type LineRow } from '$lib/git/diff-rows';
  import { commentOnLineClick, type PickHandlers } from '$lib/git/comment-range';
  import type { DiffLineKind } from '$lib/ipc/bindings/DiffLineKind';
  import type { HighlightedLine } from '$lib/markdown/highlight';
  import { cx } from '$lib/utils/cx';

  import { DIFF_ROW, DIFF_TEXT } from './diff-tones';
  import DiffViewAddComment from './DiffViewAddComment.svelte';
  import DiffViewCode from './DiffViewCode.svelte';
  import DiffViewGutter from './DiffViewGutter.svelte';

  interface Props {
    row: LineRow;
    tokens?: HighlightedLine;
    /** The line is inside the range being commented on. */
    selected: boolean;
    pick: PickHandlers;
  }

  let { row, tokens, selected, pick }: Props = $props();

  /** A removed line can only be commented on its old side; every other line on its new side. */
  const side = $derived(row.line.kind === 'remove' ? 'old' : 'new');


</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -- the gutter buttons are the keyboard path; clicking the line is a pointer shortcut -->
<div
  class={cx('group/line flex cursor-pointer items-start', selected ? 'bg-accent-wash' : DIFF_ROW[row.line.kind])}
  data-line={row.index}
  data-kind={row.line.kind}
  onclick={(event) => commentOnLineClick(event, pick, row.index, side)}
>
  <DiffViewGutter number={row.line.oldNo} side="old" index={row.index} {pick} />
  <DiffViewGutter number={row.line.newNo} side="new" index={row.index} {pick} />
  <DiffViewAddComment sign={lineSign(row.line.kind)} tone={DIFF_TEXT[row.line.kind]} index={row.index} {side} {pick} />
  <span class="sr-only">{lineSpoken(row.line.kind)}</span>
  <DiffViewCode text={row.line.text} {tokens} kind={row.line.kind} />
</div>
