<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { anchorIndex, commentHeading, coveredIndexes, rangeTarget, type PickHandlers, type RangeTarget, type ThreadComment } from '$lib/git/comment-range';
  import { highlightDiff } from '$lib/git/diff-highlight';
  import { flatLines, rowIndexes, splitRows, unifiedRows, type DiffRow } from '$lib/git/diff-rows';
  import type { CommentSide } from '$lib/ipc/bindings/CommentSide';
  import type { FileDiff } from '$lib/ipc/bindings/FileDiff';
  import type { HighlightedLine } from '$lib/markdown/highlight';

  import DiffNotice from './DiffNotice.svelte';
  import DiffViewComposer from './DiffViewComposer.svelte';
  import DiffViewSplitRow from './DiffViewSplitRow.svelte';
  import DiffViewThread from './DiffViewThread.svelte';
  import DiffViewUnifiedRow from './DiffViewUnifiedRow.svelte';
  import VirtualList from './VirtualList.svelte';

  interface Props {
    diff: FileDiff;
    mode: 'unified' | 'split';
    /** Comments saved for this file; the ones whose lines this diff shows appear under those lines. */
    comments: ThreadComment[];
    /** Longest comment body in bytes. */
    maxBytes: number;
    oncomment: (target: RangeTarget, body: string) => void;
    onedit: (id: number, body: string) => void;
    onremove: (id: number) => void;
    /** Opens the file in the file manager, for the notice shown instead of a binary diff. */
    onreveal?: () => void;
  }

  let { diff, mode, comments, maxBytes, oncomment, onedit, onremove, onreveal }: Props = $props();

  const ROW_ESTIMATE_PX = 20;
  const OVERSCAN_ROWS = 20;

  interface Selection extends RangeTarget {
    from: number;
    to: number;
  }

  const lines = $derived(flatLines(diff));
  const rows = $derived<DiffRow[]>(mode === 'split' ? splitRows(diff) : unifiedRows(diff));
  const rowsByKey = $derived(new Map(rows.map((row) => [row.key, row])));

  let highlighted = $state.raw<ReadonlyMap<number, HighlightedLine>>(new Map());
  let selection = $state.raw<Selection | null>(null);
  let composing = $state(false);
  let dragging = false;
  let anchor: { index: number; side: CommentSide } | null = null;
  let stickyHeader = $state<string | null>(null);
  let frame = 0;

  // Syntax colors arrive after the first paint; a result for a diff that was replaced is dropped.
  $effect(() => {
    const current = diff;
    let live = true;

    highlighted = new Map();
    void highlightDiff(current).then((tokens) => {
      if (live) {
        highlighted = tokens;
      }
    });

    return () => {
      live = false;
    };
  });

  // A different file starts with no half-made selection.
  $effect(() => {
    void diff;
    selection = null;
    composing = false;
    stickyHeader = null;
    anchor = null;
  });

  const covered = $derived(selection ? coveredIndexes(lines, selection) : new Set<number>());
  const composeAt = $derived(covered.size > 0 ? Math.max(...covered) : -1);

  const threadsAt = $derived.by(() => {
    const byIndex = new Map<number, ThreadComment[]>();

    for (const comment of comments) {
      const index = anchorIndex(lines, comment.side, comment.endLine);
      if (index !== -1) {
        byIndex.set(index, [...(byIndex.get(index) ?? []), comment]);
      }
    }

    return byIndex;
  });

  function threadFor(row: DiffRow): ThreadComment[] {
    return rowIndexes(row).flatMap((index) => threadsAt.get(index) ?? []);
  }

  function select(from: number, to: number, side: CommentSide): void {
    const target = rangeTarget(lines, from, to, side);
    selection = target ? { ...target, from, to } : null;
  }

  const pick: PickHandlers = {
    start(index, side, extend) {
      if (extend && anchor && anchor.side === side) {
        select(anchor.index, index, side);
      } else {
        anchor = { index, side };
        select(index, index, side);
      }

      composing = false;
      dragging = true;
    },

    extend(index, side) {
      if (dragging && anchor && anchor.side === side) {
        select(anchor.index, index, side);
      }
    },

    finish() {
      if (dragging) {
        dragging = false;
        composing = selection !== null;
      }
    },
  };

  function save(body: string): void {
    if (selection) {
      const { side, startLine, endLine, snippet } = selection;
      oncomment({ side, startLine, endLine, snippet }, body);
    }

    cancel();
  }

  function cancel(): void {
    selection = null;
    composing = false;
    anchor = null;
  }

  /** The hunk header of the topmost visible row, shown while that header has scrolled out of view. */
  function trackHunk(event: Event): void {
    const scroller = event.target;
    if (frame !== 0 || !(scroller instanceof HTMLElement) || scroller.getAttribute('role') !== 'list') {
      return;
    }

    frame = requestAnimationFrame(() => {
      frame = 0;
      const top = scroller.getBoundingClientRect().top;
      const first = [...scroller.querySelectorAll<HTMLElement>('[role="listitem"]')].find((item) => item.getBoundingClientRect().bottom > top + 1);
      const row = first ? rowsByKey.get(first.dataset.key ?? '') : undefined;

      stickyHeader = row && row.kind !== 'hunk' ? (diff.hunks[row.hunk]?.header ?? null) : null;
    });
  }

  /** Binary, withheld and rename-only files have no lines to show. */
  const noLines = $derived(diff.isBinary || diff.hunks.length === 0);
</script>

<svelte:window onpointerup={pick.finish} onpointercancel={pick.finish} />

{#if noLines}
  <DiffNotice {diff} {onreveal} />
{:else}
  <div class="relative flex size-full min-h-0 flex-col" onscrollcapture={trackHunk}>
    {#if stickyHeader}
      <div class="pointer-events-none absolute inset-x-0 top-0 z-10 truncate border-b border-line bg-surface-3 px-3 py-1 font-mono text-xs text-fg-2" aria-hidden="true">{stickyHeader}</div>
    {/if}

    <VirtualList items={rows} variable estimate={ROW_ESTIMATE_PX} overscan={OVERSCAN_ROWS} label={t('components.diff.changesIn', { path: diff.path })} key={(row) => row.key} class="min-h-0 flex-1">
      {#snippet row(item)}
        {#if item.kind === 'hunk'}
          <div class="border-y border-line bg-info-wash px-3 py-1 font-mono text-xs text-fg-2">{item.header}</div>
        {:else if item.kind === 'note'}
          <p class="px-3 py-0.5 pl-28 text-xs text-fg-3 italic">{t('components.diff.noNewline')}</p>
        {:else if item.kind === 'pair'}
          <DiffViewSplitRow row={item} {highlighted} {covered} {pick} />
        {:else}
          <DiffViewUnifiedRow row={item} tokens={highlighted.get(item.index)} selected={covered.has(item.index)} {pick} />
        {/if}

        {@const thread = threadFor(item)}
        {#if thread.length > 0}
          <div class="px-3 py-2"><DiffViewThread comments={thread} {maxBytes} {onedit} {onremove} /></div>
        {/if}

        {#if composing && selection && rowIndexes(item).includes(composeAt)}
          <div class="px-3 py-2"><DiffViewComposer heading={commentHeading(selection)} {maxBytes} onsave={save} oncancel={cancel} /></div>
        {/if}
      {/snippet}
    </VirtualList>

    {#if diff.truncated}
      <p class="border-t border-line bg-warning-wash px-3 py-1.5 text-xs text-fg" role="status">{t('components.diff.truncated')}</p>
    {/if}
  </div>
{/if}
