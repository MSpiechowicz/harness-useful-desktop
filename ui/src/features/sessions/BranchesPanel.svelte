<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Cloud from '@lucide/svelte/icons/cloud';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Laptop from '@lucide/svelte/icons/laptop';

  import Badge from '$lib/components/Badge.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { cx } from '$lib/utils/cx';
  import { relativeTime } from '$lib/utils/format';

  let open = $state(true);

  const local = $derived(git.branches.filter((branch) => !branch.isRemote));
  const remote = $derived(git.branches.filter((branch) => branch.isRemote));

  /** Splits `origin/fix/x` into the remote (shown muted) and the branch, so the branch reads first. */
  function splitRemote(name: string): [string, string] {
    const slash = name.indexOf('/');

    return slash > 0 ? [name.slice(0, slash + 1), name.slice(slash + 1)] : ['', name];
  }
</script>

<section aria-label="Git branches" class={cx('flex min-h-0 flex-col border-t border-line', open && 'max-h-[45%]')}>
  <!-- Room above and below, so the hover wash never touches the divider lines. -->
  <div class="flex items-center px-1 py-1">
    <button type="button" aria-expanded={open} class="flex h-9 flex-1 items-center gap-1.5 rounded-md px-2 text-sm font-medium text-fg hover:bg-hover active:bg-press" onclick={() => (open = !open)}>
      <ChevronRight size={16} class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} aria-hidden="true" />
      <GitBranch size={16} class="text-accent-text" aria-hidden="true" />
      Git branches
      {#if git.status?.isRepo}<span class="tabular rounded-full bg-surface-3 px-1.5 text-xs leading-5 text-fg-2">{git.branches.length}</span>{/if}
    </button>
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
        {#each [{ title: 'Local', icon: Laptop, branches: local }, { title: 'Remote', icon: Cloud, branches: remote }] as group, index (group.title)}
          {#if group.branches.length > 0}
            <!-- Each group opens with a clear header; the second one is set apart by a divider. -->
            <h3 class={cx('flex items-center gap-2 px-2 pb-2 text-xs font-semibold tracking-wider text-fg-2 uppercase', index > 0 && local.length > 0 ? 'mt-3 border-t border-line pt-4' : 'pt-2')}>
              <group.icon size={14} class="text-fg-3" aria-hidden="true" />
              {group.title}
              <span class="tabular rounded-full bg-surface-3 px-1.5 text-[11px] leading-[18px] font-medium tracking-normal text-fg-3 normal-case">{group.branches.length}</span>
            </h3>
            <ul aria-label="{group.title} branches" class="flex flex-col gap-1">
              {#each group.branches as branch (branch.name)}
                {@const [prefix, rest] = branch.isRemote ? splitRemote(branch.name) : ['', branch.name]}
                <li
                  class={cx('flex flex-col gap-1 rounded-md px-2.5 py-2', branch.isCurrent && 'bg-accent-wash')}
                  aria-current={branch.isCurrent ? 'true' : undefined}
                >
                  <span class="flex items-center gap-2">
                    <span class="min-w-0 flex-1 truncate font-mono text-[13px] text-fg" title={branch.name}>{#if prefix}<span class="text-fg-3">{prefix}</span>{/if}{rest}</span>
                    {#if branch.isCurrent}<Badge tone="accent">Current</Badge>{/if}
                    {#if branch.ahead}<span class="tabular text-xs text-success" title="{branch.ahead} ahead">↑{branch.ahead}</span>{/if}
                    {#if branch.behind}<span class="tabular text-xs text-warning" title="{branch.behind} behind">↓{branch.behind}</span>{/if}
                  </span>
                  {#if branch.lastCommit}
                    <span class="flex min-w-0 gap-1.5 text-xs text-fg-3">
                      <span class="min-w-0 truncate">{branch.lastCommit.summary}</span>
                      <span class="shrink-0 whitespace-nowrap">· {relativeTime(branch.lastCommit.date)}</span>
                    </span>
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
