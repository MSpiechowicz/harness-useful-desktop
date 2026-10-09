<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import GitBranch from '@lucide/svelte/icons/git-branch';

  import type { GitStatus } from '$lib/ipc/bindings/GitStatus';

  interface Props {
    status: GitStatus;
  }

  let { status }: Props = $props();

  const name = $derived(status.branch ?? (status.head ? `detached at ${status.head}` : 'no commits yet'));
</script>

<span class="inline-flex min-w-0 items-center gap-2 text-sm text-fg">
  <GitBranch size={16} class="shrink-0 text-accent-text" aria-hidden="true" />
  <span class="truncate font-medium" title={status.upstream ? `Tracks ${status.upstream}` : undefined}>{name}</span>

  {#if status.ahead > 0}
    <span class="tabular inline-flex items-center text-xs text-success" title="{status.ahead} ahead of {status.upstream ?? 'upstream'}">
      <ArrowUp size={16} aria-hidden="true" />{status.ahead}<span class="sr-only">{' ahead'}</span>
    </span>
  {/if}
  {#if status.behind > 0}
    <span class="tabular inline-flex items-center text-xs text-warning" title="{status.behind} behind {status.upstream ?? 'upstream'}">
      <ArrowDown size={16} aria-hidden="true" />{status.behind}<span class="sr-only">{' behind'}</span>
    </span>
  {/if}
</span>
