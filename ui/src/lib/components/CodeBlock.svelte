<script lang="ts">
  import { highlight, type HighlightedLine } from '$lib/markdown/highlight';
  import { cx } from '$lib/utils/cx';

  import CopyButton from './CopyButton.svelte';

  interface Props {
    code: string;
    /** Fence label such as `ts` or `rust`; unknown labels stay plain. */
    lang?: string;
    /** The block is still growing: highlighting waits for a pause in the stream. */
    streaming?: boolean;
    class?: string;
  }

  let { code, lang = '', streaming = false, class: className }: Props = $props();

  const STREAM_PAUSE_MS = 150;

  let highlighted = $state.raw<{ code: string; lines: HighlightedLine[] } | null>(null);

  // Highlight lazily and off the render path; a result for stale code is discarded.
  $effect(() => {
    const source = code;
    const label = lang;
    let current = true;

    const timer = setTimeout(
      async () => {
        const lines = await highlight(source, label);
        if (current && lines) {
          highlighted = { code: source, lines };
        }
      },
      streaming ? STREAM_PAUSE_MS : 0,
    );

    return () => {
      current = false;
      clearTimeout(timer);
    };
  });

  /** Colored lines for the code highlighted so far, plus any text appended since (plain until the next pass). */
  const view = $derived(highlighted && code.startsWith(highlighted.code) ? { lines: highlighted.lines, tail: code.slice(highlighted.code.length) } : null);
</script>

<figure class={cx('overflow-hidden rounded-md border border-line bg-canvas', className)}>
  <figcaption class="flex h-8 items-center justify-between border-b border-line pr-1 pl-3.5 text-xs text-fg-3">
    <span class="font-mono">{lang || 'text'}</span>
    <CopyButton text={code} label="Copy code" />
  </figcaption>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
  <pre class="max-h-[28rem] overflow-auto px-3.5 py-3 font-mono text-[13px] leading-relaxed text-fg" tabindex="0"><code>{#if view}{#each view.lines as line, row (row)}{#each line as token, column (column)}<span style:color={token.color} class:font-semibold={token.bold} class:italic={token.italic}>{token.content}</span>{/each}{#if row < view.lines.length - 1}{'\n'}{/if}{/each}{view.tail}{:else}{code}{/if}</code></pre>
</figure>
