<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import GitBranch from '@lucide/svelte/icons/git-branch';

  import { t } from '$lib/i18n/i18n.svelte';
  import type { GitStatus } from '$lib/ipc/bindings/GitStatus';

  interface Props {
    status: GitStatus;
  }

  let { status }: Props = $props();

  const name = $derived(status.branch ?? (status.head ? t('changes.branch.detached', { head: status.head }) : t('changes.branch.noCommits')));
  const upstream = $derived(status.upstream ?? t('changes.branch.upstream'));
</script>

<span class="inline-flex min-w-0 items-center gap-2 text-sm text-fg">
  <GitBranch size={16} class="shrink-0 text-accent-text" aria-hidden="true" />
  <span class="truncate font-medium" title={status.upstream ? t('changes.branch.tracks', { upstream: status.upstream }) : undefined}>{name}</span>

  {#if status.ahead > 0}
    <span class="tabular inline-flex items-center text-xs text-success" title={t('changes.branch.ahead', { count: status.ahead, upstream })}>
      <ArrowUp size={16} aria-hidden="true" />{status.ahead}<span class="sr-only">{` ${t('changes.branch.aheadWord')}`}</span>
    </span>
  {/if}
  {#if status.behind > 0}
    <span class="tabular inline-flex items-center text-xs text-warning" title={t('changes.branch.behind', { count: status.behind, upstream })}>
      <ArrowDown size={16} aria-hidden="true" />{status.behind}<span class="sr-only">{` ${t('changes.branch.behindWord')}`}</span>
    </span>
  {/if}
</span>
