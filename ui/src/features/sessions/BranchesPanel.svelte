<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import Badge from '$lib/components/Badge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { cx } from '$lib/utils/cx';
  import { relativeTime } from '$lib/utils/format';

  let open = $state(true);

  const local = $derived(git.branches.filter((branch) => !branch.isRemote));
  const remote = $derived(git.branches.filter((branch) => branch.isRemote));
</script>

<section aria-label="Git branches" class={cx('flex min-h-0 flex-col border-t border-line', open && 'max-h-[45%]')}>
  <div class="flex items-center gap-1 pr-2 pl-1">
    <button type="button" aria-expanded={open} class="flex h-9 flex-1 items-center gap-1.5 rounded-md px-2 text-sm font-medium text-fg hover:bg-hover active:bg-press" onclick={() => (open = !open)}>
      <ChevronRight size={16} class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} aria-hidden="true" />
      <GitBranch size={16} class="text-accent-text" aria-hidden="true" />
      Git branches
      {#if git.status?.isRepo}<span class="tabular rounded-full bg-surface-3 px-1.5 text-xs leading-5 text-fg-2">{git.branches.length}</span>{/if}
    </button>
    <IconButton icon={RotateCw} label="Refresh branches" size="sm" disabled={git.refreshing} onclick={() => void git.refresh()} />
  </div>

  {#if open}
    <div class="min-h-0 overflow-y-auto px-2 pb-2">
      {#if git.status && !git.status.isRepo}
        <p class="px-2 py-3 text-sm text-fg-3" role="status">This project is not a git repository.</p>
      {:else if git.error && !git.status}
        <p class="px-2 py-3 text-sm text-danger" role="alert">{git.error.message}</p>
      {:else if git.branches.length === 0}
        <p class="px-2 py-3 text-sm text-fg-3" role="status">{git.status ? 'No branches yet.' : 'Reading branches…'}</p>
      {:else}
        {#each [{ title: 'Local', branches: local }, { title: 'Remote', branches: remote }] as group (group.title)}
          {#if group.branches.length > 0}
            <h3 class="px-2 pt-2 pb-1 text-xs font-medium tracking-wider text-fg-3 uppercase">{group.title}</h3>
            <ul aria-label="{group.title} branches" class="flex flex-col">
              {#each group.branches as branch (branch.name)}
                <li class={cx('flex flex-col gap-0.5 rounded-md px-2 py-1.5', branch.isCurrent && 'bg-accent-wash')} aria-current={branch.isCurrent ? 'true' : undefined}>
                  <span class="flex items-center gap-2">
                    <span class="min-w-0 flex-1 truncate font-mono text-sm text-fg" title={branch.name}>{branch.name}</span>
                    {#if branch.isCurrent}<Badge tone="accent">Current</Badge>{/if}
                    {#if branch.ahead}<span class="tabular text-xs text-success" title="{branch.ahead} ahead">↑{branch.ahead}</span>{/if}
                    {#if branch.behind}<span class="tabular text-xs text-warning" title="{branch.behind} behind">↓{branch.behind}</span>{/if}
                  </span>
                  {#if branch.lastCommit}
                    <span class="truncate text-xs text-fg-3">{branch.lastCommit.summary} · {relativeTime(branch.lastCommit.date)}</span>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/each}
      {/if}
    </div>
  {/if}
</section>
