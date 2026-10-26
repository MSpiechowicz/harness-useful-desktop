<script lang="ts">
  import GitBranch from '@lucide/svelte/icons/git-branch';

  import StatusDot from '$lib/components/StatusDot.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { BRANCH_AGE_TICK_MS } from '$lib/stores/live';
  import { cx } from '$lib/utils/cx';
  import { relativeTime } from '$lib/utils/format';

  interface Props {
    /** The branch, or null on a detached HEAD. */
    branch: string | null;
    /** The remote the branch syncs with; null without one, and then there is nothing to say about checking it. */
    remote: string | null;
    /** When the remote was last fetched (RFC 3339), if that is known. */
    lastFetched: string | null;
    /** The last fetch of the app could not reach the remote. */
    failed: boolean;
  }

  let { branch, remote, lastFetched, failed }: Props = $props();

  let now = $state(Date.now());
  const status = $derived(remote !== null && (failed || lastFetched !== null));

  // The age counts on while the menu stays open.
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), BRANCH_AGE_TICK_MS);

    return () => clearInterval(timer);
  });
</script>

<div class="flex items-stretch gap-3 px-3.5 pt-3.5 pb-2.5">
  <span class="grid size-10 shrink-0 place-items-center rounded-sm bg-surface-3 text-fg-2"><GitBranch size={18} aria-hidden="true" /></span>

  <!-- Two lines that together are as tall as the tile. -->
  <div class={cx('flex h-10 min-w-0 flex-1 flex-col', status ? 'justify-between' : 'justify-center')}>
    {#if branch}
      <div class="h-5 truncate font-mono text-base leading-5 font-semibold">{branch}</div>
    {:else}
      <div class="h-5 truncate text-base leading-5 font-semibold">{t('chat.footer.detached')}</div>
    {/if}

    {#if remote && failed}
      <div class="flex h-4.5 items-center gap-2 text-xs leading-4.5 text-warning">
        <StatusDot tone="warning" />
        <span class="min-w-0 truncate">{t('branch.header.unreachable', { remote })}</span>
      </div>
    {:else if remote && lastFetched}
      <div class="flex h-4.5 items-center gap-2 text-xs leading-4.5 text-fg-3">
        <span class="live-dot shrink-0 text-success" aria-hidden="true"></span>
        <span class="tabular min-w-0 truncate">{t('branch.header.checked', { remote, time: relativeTime(lastFetched, now) })}</span>
      </div>
    {/if}
  </div>
</div>

