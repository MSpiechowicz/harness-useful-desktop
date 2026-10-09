<script lang="ts">
  import type { HighlightedLine } from '$lib/markdown/highlight';

  interface Props {
    text: string;
    /** Syntax colors for the line, once they have been computed. Always rendered as text. */
    tokens?: HighlightedLine;
  }

  let { text, tokens }: Props = $props();
</script>

<span class="min-w-0 flex-1 px-2 font-mono text-[13px] leading-[18px] break-all whitespace-pre-wrap text-fg">
  {#if tokens}{#each tokens as token, column (column)}<span style:color={token.color} class:font-semibold={token.bold} class:italic={token.italic}>{token.content}</span>{/each}{:else}{text}{/if}
</span>
