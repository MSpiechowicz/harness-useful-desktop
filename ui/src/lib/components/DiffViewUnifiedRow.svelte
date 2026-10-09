<script lang="ts">
  import { lineSign, lineSpoken, type LineRow } from '$lib/git/diff-rows';
  import type { PickHandlers } from '$lib/git/comment-range';
  import type { DiffLineKind } from '$lib/ipc/bindings/DiffLineKind';
  import type { HighlightedLine } from '$lib/markdown/highlight';
  import { cx } from '$lib/utils/cx';

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

  const TONES: Record<DiffLineKind, string> = { add: 'bg-success-wash', remove: 'bg-danger-wash', context: '', noNewline: '' };
</script>

<div class={cx('flex items-start', selected ? 'bg-accent-wash' : TONES[row.line.kind])} data-line={row.index} data-kind={row.line.kind}>
  <DiffViewGutter number={row.line.oldNo} side="old" index={row.index} {pick} />
  <DiffViewGutter number={row.line.newNo} side="new" index={row.index} {pick} />
  <span class="w-4 shrink-0 text-center font-mono text-[13px] leading-[18px] text-fg-3 select-none" aria-hidden="true">{lineSign(row.line.kind)}</span>
  <span class="sr-only">{lineSpoken(row.line.kind)}</span>
  <DiffViewCode text={row.line.text} {tokens} />
</div>
