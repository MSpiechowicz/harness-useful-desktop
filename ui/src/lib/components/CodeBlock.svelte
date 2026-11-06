<script lang="ts">
  import { HIGHLIGHT_LOOKAHEAD_PX } from '$lib/design/layout';
  import { t } from '$lib/i18n/i18n.svelte';
  import { highlight, type HighlightedLine } from '$lib/markdown/highlight';
  import { grammarFor } from '$lib/markdown/languages';
  import { cx } from '$lib/utils/cx';

  import CopyButton from './CopyButton.svelte';
  import { DIFF_ROW, DIFF_TEXT } from './diff-tones';

  interface Props {
    code: string;
    /** Fence label such as `ts` or `rust`; unknown labels stay plain. */
    lang?: string;
    /** The block is still growing: highlighting waits for a pause in the stream. */
    streaming?: boolean;
    /** The code scrolls inside the block past `--code-max-h`, under a header that stays: for a panel or a dialog that holds a whole script. Chat blocks show in full. */
    capped?: boolean;
    class?: string;
  }

  let { code, lang = '', streaming = false, capped = false, class: className }: Props = $props();

  const CAP = 'max-h-[var(--code-max-h)] overflow-y-auto';

  const STREAM_PAUSE_MS = 150;

  let highlighted = $state.raw<{ code: string; lines: HighlightedLine[] } | null>(null);

  // Highlighting runs on the UI thread, and a review prompt can hold a hundred blocks: only those near the screen pay for it, once.
  let near = $state(typeof IntersectionObserver === 'undefined');

  function whenNear(element: HTMLElement): { destroy: () => void } | undefined {
    if (near) {
      return undefined;
    }

    const margin = `${HIGHLIGHT_LOOKAHEAD_PX}px`;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          near = true;
          observer.disconnect();
        }
      },
      // `rootMargin` reaches past the window, `scrollMargin` past the scrolling list the block sits in.
      { rootMargin: margin, scrollMargin: margin },
    );
    observer.observe(element);

    return { destroy: () => observer.disconnect() };
  }

  // Highlight lazily and off the render path; a result for stale code is discarded.
  $effect(() => {
    if (!near) {
      return;
    }

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
  /** Diffs tint whole lines (added, removed, hunk header), across the full scroll width. */
  const isDiff = $derived(grammarFor(lang) === 'diff');

  function diffKind(line: string): 'add' | 'remove' | 'hunk' | null {
    if (line.startsWith('@@')) {
      return 'hunk';
    }
    if (line.startsWith('+') && !line.startsWith('+++')) {
      return 'add';
    }
    if (line.startsWith('-') && !line.startsWith('---')) {
      return 'remove';
    }

    return null;
  }

  const view = $derived(highlighted && code.startsWith(highlighted.code) ? { lines: highlighted.lines, tail: code.slice(highlighted.code.length) } : null);
</script>

<figure use:whenNear class={cx('chat-radius overflow-hidden border border-line bg-canvas', className)}>
  <!-- pr-[9px] + the copy icon's 5px inset inside its 28px button = the label's 14px on the left. -->
  <figcaption class="flex h-8 items-center justify-between border-b border-line bg-surface-2 pr-[9px] pl-3.5 text-xs text-fg-2">
    <span class="font-mono font-medium">{lang || 'text'}</span>
    <CopyButton text={code} label={t('components.copy.code')} />
  </figcaption>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
  <!-- Long lines wrap and the block shows in full: the conversation is the only scroller, so no scrollbar ever sits
       over the code (WebKitGTK widens its overlay bars on hover). Only a `capped` block scrolls inside itself. Copy keeps the original lines. -->
  {#if isDiff}
    <pre class={cx('py-3 font-mono text-[13px] leading-5 [overflow-wrap:anywhere] whitespace-pre-wrap text-fg', capped && CAP)} tabindex={capped ? 0 : undefined}><code class="grid">{#each code.split('\n') as line, row (row)}{@const kind = diffKind(line)}<span class={cx('px-3.5', kind === 'hunk' && 'text-info', kind !== null && kind !== 'hunk' && `${DIFF_ROW[kind]} ${DIFF_TEXT[kind]}`)}>{line || ' '}</span>{/each}</code></pre>
  {:else}
  <pre class={cx('px-3.5 py-3 font-mono text-[13px] leading-5 [overflow-wrap:anywhere] whitespace-pre-wrap text-fg', capped && CAP)} tabindex={capped ? 0 : undefined}><code>{#if view}{#each view.lines as line, row (row)}{#each line as token, column (column)}<span style:color={token.color} class:font-semibold={token.bold} class:italic={token.italic}>{token.content}</span>{/each}{#if row < view.lines.length - 1}{'\n'}{/if}{/each}{view.tail}{:else}{code}{/if}</code></pre>
  {/if}
</figure>
