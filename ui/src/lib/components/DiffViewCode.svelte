<script lang="ts">
  import type { DiffLineKind } from '$lib/ipc/bindings/DiffLineKind';
  import type { HighlightedLine } from '$lib/markdown/highlight';
  import { cx } from '$lib/utils/cx';

  import { DIFF_TEXT } from './diff-tones';

  interface Props {
    text: string;
    /** Syntax colors for the line, once they have been computed. Always rendered as text. */
    tokens?: HighlightedLine;
    /** Added and removed lines take the chat's diff green and red for their plain text. */
    kind?: DiffLineKind;
  }

  let { text, tokens, kind = 'context' }: Props = $props();

  /** Plain text keeps the line's color; real syntax colors (keywords, strings…) stay on top of it. */
  function colorOf(color: string | undefined): string | undefined {
    return color === undefined || color.includes('--shiki-foreground') ? undefined : color;
  }
</script>

<span class={cx('min-w-0 flex-1 px-2 font-mono text-[14px] leading-[20px] break-all whitespace-pre-wrap', DIFF_TEXT[kind])}>
  {#if tokens}{#each tokens as token, column (column)}<span style:color={colorOf(token.color)} class:font-semibold={token.bold} class:italic={token.italic}>{token.content}</span>{/each}{:else}{text}{/if}
</span>
