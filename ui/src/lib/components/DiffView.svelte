<script lang="ts">
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import { tick, untrack } from 'svelte';

  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { diffKey, expandGaps, skippedBefore } from '$lib/git/diff-expand';
  import { matchingLines, normalizeQuery } from '$lib/git/search';
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
    /** Reads the file's text so the unchanged lines between hunks can be opened; without it those rows only count. */
    loadLines?: () => Promise<string[]>;
    /** What the user searched for; its matches are marked, and `cursor` picks the one that is scrolled to. */
    query?: string;
    cursor?: number;
    /** Told how many lines match, so the toolbar can say "3 of 12". */
    onmatches?: (count: number) => void;
    oncomment: (target: RangeTarget, body: string) => void;
    onedit: (id: number, body: string) => void;
    onremove: (id: number) => void;
    /** Opens the file in the file manager, for the notice shown instead of a binary diff. */
    onreveal?: () => void;
  }

  let { diff: source, mode, comments, maxBytes, query = '', cursor = 0, onmatches, loadLines, oncomment, onedit, onremove, onreveal }: Props = $props();

  const ROW_ESTIMATE_PX = 20;
  const OVERSCAN_ROWS = 20;

  interface Selection extends RangeTarget {
    from: number;
    to: number;
  }

  let fileText = $state.raw<string[] | null>(null);
  let opened = $state.raw<ReadonlySet<number>>(new Set());
  let readingFile = $state(false);
  let readFailed = $state(false);

  const diff = $derived(expandGaps(source, fileText, opened));
  const lines = $derived(flatLines(diff));
  const rows = $derived<DiffRow[]>(mode === 'split' ? splitRows(diff) : unifiedRows(diff));

  let highlighted = $state.raw<ReadonlyMap<number, HighlightedLine>>(new Map());
  let selection = $state.raw<Selection | null>(null);
  let composing = $state(false);
  let dragging = false;
  /** A shift-click marked the start of a range; the composer waits for the second click that ends it. */
  let awaitingEnd = false;
  let anchor: { index: number; side: CommentSide } | null = null;

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

  // A different file (or the same one, changed) starts closed and with no half-made selection.
  $effect(() => {
    void diffKey(source);
    untrack(() => {
      opened = new Set();
      fileText = null;
      readFailed = false;
    });
  });

  // Opening a gap renumbers the lines under any selection, so it goes with the old numbers.
  $effect(() => {
    void [diffKey(source), opened];
    selection = null;
    composing = false;
    anchor = null;
    awaitingEnd = false;
  });

  const needle = $derived(normalizeQuery(query));
  const matches = $derived(needle === null ? [] : matchingLines(diff, needle));
  const currentMatch = $derived(matches[cursor] ?? -1);
  let list: { scrollToIndex: (index: number) => void } | undefined = $state();

  $effect(() => {
    onmatches?.(matches.length);
  });

  // Stepping to a match brings its row to the top of the view.
  $effect(() => {
    const target = currentMatch;
    if (target === -1) {
      return;
    }

    const at = rows.findIndex((row) => rowIndexes(row).includes(target));
    if (at !== -1) {
      void tick().then(() => list?.scrollToIndex(at));
    }
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

  /** Reads the file once, then opens the lines before `hunk`. */
  async function openGap(hunk: number): Promise<void> {
    if (readingFile || !loadLines) {
      return;
    }

    if (fileText === null) {
      readingFile = true;
      readFailed = false;

      try {
        fileText = await loadLines();
      } catch {
        readFailed = true;

        return;
      } finally {
        readingFile = false;
      }
    }

    opened = new Set([...opened, hunk]);
  }

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
        awaitingEnd = false;
      } else {
        anchor = { index, side };
        select(index, index, side);
        awaitingEnd = extend;
      }

      composing = false;
      dragging = true;
    },

    extend(index, side) {
      if (dragging && anchor && anchor.side === side) {
        select(anchor.index, index, side);
        awaitingEnd = false;
      }
    },

    finish() {
      if (dragging) {
        dragging = false;
        composing = selection !== null && !awaitingEnd;
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
    awaitingEnd = false;
  }

  /** Binary, withheld and rename-only files have no lines to show. */
  const noLines = $derived(diff.isBinary || diff.hunks.length === 0);
</script>

<svelte:window onpointerup={pick.finish} onpointercancel={pick.finish} />

{#if noLines}
  <DiffNotice {diff} {onreveal} />
{:else}
  <div class="relative flex size-full min-h-0 flex-col">
    <VirtualList bind:this={list} items={rows} variable estimate={ROW_ESTIMATE_PX} overscan={OVERSCAN_ROWS} label={t('components.diff.changesIn', { path: diff.path })} key={(row) => row.key} class="min-h-0 flex-1">
      {#snippet row(item)}
        {#if item.kind === 'hunk'}
          <!-- Git's header (a line range and a guessed nearby line of code) says nothing the gutter does not. What a reader
               wants to know is how much of the file lies between two changes, so that is all the row says. -->
          {@const skipped = skippedBefore(diff, item.hunk)}
          {#if skipped > 0}
            {#if loadLines}
              <button
                type="button"
                disabled={readingFile}
                class="flex h-8 w-full items-center justify-center gap-2 border-y border-line bg-surface-2 text-xs text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-surface-3 hover:text-fg focus-visible:outline-offset-[-2px] disabled:cursor-progress"
                onclick={() => void openGap(item.hunk)}
              >
                <ChevronsUpDown size={14} aria-hidden="true" />{readFailed ? t('components.diff.unreadable') : tn('components.diff.showSkipped', skipped)}
              </button>
            {:else}
              <div class="flex h-8 items-center justify-center gap-2 border-y border-line bg-surface-2 text-xs text-fg-3">
                <ChevronsUpDown size={14} aria-hidden="true" />{tn('components.diff.skipped', skipped)}
              </div>
            {/if}
          {/if}
        {:else if item.kind === 'note'}
          <p class="px-3 py-0.5 pl-28 text-xs text-fg-3 italic">{t('components.diff.noNewline')}</p>
        {:else if item.kind === 'pair'}
          <DiffViewSplitRow row={item} {highlighted} {covered} {pick} query={needle} {currentMatch} />
        {:else}
          <DiffViewUnifiedRow row={item} tokens={highlighted.get(item.index)} selected={covered.has(item.index)} {pick} query={needle} {currentMatch} />
        {/if}

        {@const thread = threadFor(item)}
        {#if thread.length > 0}
          <div class="py-2 pr-3 pl-[98px]"><DiffViewThread comments={thread} {maxBytes} {onedit} {onremove} /></div>
        {/if}

        {#if composing && selection && rowIndexes(item).includes(composeAt)}
          <div class="py-2 pr-3 pl-[98px]"><DiffViewComposer heading={commentHeading(selection)} {maxBytes} onsave={save} oncancel={cancel} /></div>
        {/if}
      {/snippet}
    </VirtualList>

    {#if diff.truncated}
      <p class="border-t border-line bg-warning-wash px-3 py-1.5 text-xs text-fg" role="status">{t('components.diff.truncated')}</p>
    {/if}
  </div>
{/if}
