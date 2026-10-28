<script lang="ts">
  import Check from '@lucide/svelte/icons/check';

  import Spinner from '$lib/components/Spinner.svelte';
  import { rowKey } from '$lib/git/branch-keys';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { BranchRow } from '$lib/ipc/bindings/BranchRow';
  import { branchSync } from '$lib/stores/branch-sync.svelte';
  import { relativeTime } from '$lib/utils/format';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The remote that remote-only branches live on. */
    remote: string | null;
    /** The row that is chosen (the branch checked out, the main branch): it gets the check mark. */
    selected: (row: BranchRow) => boolean;
    onpick: (row: BranchRow) => void;
    disabled?: boolean;
    /** Say so when nothing matches. Off while the menu offers to create the branch that was typed. */
    showEmpty?: boolean;
  }

  let { remote, selected, onpick, disabled = false, showEmpty = true }: Props = $props();

  /** How far below the visible rows the next page is asked for: one box height, so it is there before the end is reached. */
  const LOOKAHEAD = '0% 0% 100% 0%';

  const page = $derived(branchSync.page);

  let scroller = $state<HTMLElement>();
  let sentinel = $state<HTMLElement>();

  // Watches the end of the list. Armed again whenever the rows change, so a page that does not fill the box
  // (or leaves the end in view) asks for the next one at once. A failed page does not change them: it waits for a new scroll.
  $effect(() => {
    void branchSync.page;
    if (!scroller || !sentinel || typeof IntersectionObserver === 'undefined') {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          void branchSync.loadMore();
        }
      },
      { root: scroller, rootMargin: LOOKAHEAD },
    );
    observer.observe(sentinel);

    return () => observer.disconnect();
  });

  /** After a failed page the end may stay in view, which the observer does not report again: a scroll near the end asks once more. */
  function retryNearEnd(): void {
    if (scroller && branchSync.searchFailed && scroller.scrollHeight - scroller.scrollTop <= 2 * scroller.clientHeight) {
      void branchSync.loadMore();
    }
  }
</script>

<!-- The branches the backend found for the search, local ones first. Scrolling near the end loads the next page. -->
<div bind:this={scroller} onscroll={retryNearEnd} class="max-h-52 overflow-y-auto px-1.5 pb-2" role="group" aria-label={t('branch.list.title')}>
  {#each page?.branches ?? [] as row (rowKey(row))}
    {@const chosen = selected(row)}
    <button
      type="button"
      role="menuitemradio"
      aria-checked={chosen}
      data-menu-item
      {disabled}
      class={cx(
        'flex h-control w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover focus-visible:-outline-offset-2 disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-transparent',
        chosen ? 'text-fg' : 'text-fg-2',
      )}
      onclick={() => onpick(row)}
    >
      <Check size={16} aria-hidden="true" class={cx('shrink-0 text-accent-text', !chosen && 'invisible')} />
      <span class="min-w-0 truncate font-mono">{row.name}</span>
      {#if row.remoteOnly && remote}
        <span class="shrink-0 text-xs text-fg-3" title={t('branch.list.remoteOnly', { remote })}>{remote}</span>
      {/if}
      {#if row.lastCommitDate}
        <span class="tabular ml-auto shrink-0 pl-2 text-xs text-fg-3">{relativeTime(row.lastCommitDate)}</span>
      {/if}
    </button>
  {/each}

  {#if showEmpty && page && page.total === 0}
    <p class="px-2 py-3 text-sm text-fg-3" role="status">{t('branch.list.none')}</p>
  {:else if branchSync.searchFailed}
    <p class="px-2 py-3 text-sm text-warning" role="status">{t('branch.list.failed')}</p>
  {/if}

  {#if branchSync.loadingMore}
    <p class="flex h-control items-center gap-2 px-2 text-sm text-fg-3" role="status">
      <Spinner size={14} class="text-accent-text" />
      <span>{t('branch.list.loading')}</span>
    </p>
  {/if}

  {#if branchSync.hasMore}
    <div bind:this={sentinel} aria-hidden="true"></div>
  {/if}
</div>
