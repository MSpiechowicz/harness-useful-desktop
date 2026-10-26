<script lang="ts">
  import Check from '@lucide/svelte/icons/check';

  import { t, tn } from '$lib/i18n/i18n.svelte';
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

  const page = $derived(branchSync.page);
  const hidden = $derived(page ? page.total - page.branches.length : 0);
</script>

<!-- The branches the backend found for the search, local ones first. A long list shows how many more match: the search narrows it. -->
<div class="max-h-52 overflow-y-auto px-1.5 pb-2" role="group" aria-label={t('branch.list.title')}>
  {#each page?.branches ?? [] as row (row.remoteOnly + row.name)}
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

  {#if hidden > 0}
    <p class="px-2 pt-1.5 text-xs text-fg-3" role="status">{t('branch.list.more', { count: hidden })}</p>
  {/if}
</div>
