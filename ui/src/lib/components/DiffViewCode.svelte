<script lang="ts">
  import type { DiffLineKind } from '$lib/ipc/bindings/DiffLineKind';
  import type { HighlightedLine } from '$lib/markdown/highlight';
  import { cx } from '$lib/utils/cx';

  import { cutPieces, matchRanges } from '$lib/git/search';

  import { DIFF_TEXT } from './diff-tones';

  interface Props {
    text: string;
    /** Syntax colors for the line, once they have been computed. Always rendered as text. */
    tokens?: HighlightedLine;
    /** Added and removed lines take the chat's diff green and red for their plain text. */
    kind?: DiffLineKind;
    /** The normalized search query; its occurrences in the line are marked. */
    query?: string | null;
    /** The line holds the match the user is stepping through, so its marks stand out. */
    current?: boolean;
  }

  let { text, tokens, kind = 'context', query = null, current = false }: Props = $props();

  const spans = $derived(query ? matchRanges(text, query) : []);

  /** Each token cut at the match edges; `offset` is where the token starts in the line. */
  const parts = $derived.by(() => {
    let offset = 0;

    return (tokens ?? []).map((token) => {
      const cut = { token, pieces: cutPieces(token.content, offset, spans) };
      offset += token.content.length;

      return cut;
    });
  });

  /** Plain text keeps the line's color; real syntax colors (keywords, strings…) stay on top of it. */
  function colorOf(color: string | undefined): string | undefined {
    return color === undefined || color.includes('--shiki-foreground') ? undefined : color;
  }
</script>

<span class={cx('min-w-0 flex-1 px-2 font-mono text-[14px] leading-[20px] break-all whitespace-pre-wrap', DIFF_TEXT[kind])}>
  {#if tokens}{#each parts as part, column (column)}<span style:color={colorOf(part.token.color)} class:font-semibold={part.token.bold} class:italic={part.token.italic}>{#each part.pieces as piece, at (at)}{#if piece.hit}<mark class={cx('rounded-xs text-inherit', current ? 'bg-warning/60 text-surface-1' : 'bg-warning/25')}>{piece.text}</mark>{:else}{piece.text}{/if}{/each}</span>{/each}{:else}{#each cutPieces(text, 0, spans) as piece, at (at)}{#if piece.hit}<mark class={cx('rounded-xs text-inherit', current ? 'bg-warning/60 text-surface-1' : 'bg-warning/25')}>{piece.text}</mark>{:else}{piece.text}{/if}{/each}{/if}
</span>
