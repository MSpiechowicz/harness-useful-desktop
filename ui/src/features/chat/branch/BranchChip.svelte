<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { branchSync } from '$lib/stores/branch-sync.svelte';
  import { git } from '$lib/stores/git.svelte';

  import BranchPopover from './BranchPopover.svelte';
  import { CHIP, CHIP_OPEN } from './chip';
  import DropCopyDialog from './DropCopyDialog.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let open = $state(false);
  let dropping = $state(false);

  // Git facts belong to the open project; a chat that sits in another one shows none. The branch state is the more exact source
  // (it still names the branch while a rebase is stopped), the git status answers until it has been read.
  const status = $derived(git.status?.isRepo && git.activeProject === session.info.project ? git.status : null);
  const sync = $derived(status && branchSync.state?.isRepo ? branchSync.state : null);
  const branch = $derived((sync ? sync.branch : status?.branch) ?? t('chat.footer.detached'));
  const ahead = $derived(sync ? sync.ahead : (status?.ahead ?? 0));
  const behind = $derived(sync ? sync.behind : (status?.behind ?? 0));
  const operation = $derived(sync?.operation ?? null);
  // Main moving on is news only while nothing is stopped and this is not main itself.
  const mainNew = $derived(sync?.main && !sync.main.isCurrent && !operation ? sync.main.newCommits : 0);
</script>

{#if status}
  <Popover label={t('branch.popover')} kind="dialog" side="top" align="start" bind:open class="w-(--branch-popover-w) p-0!" rootClass="shrink-[100]">
    {#snippet trigger({ props })}
      <button type="button" {...props} title={branch} class="{CHIP} {CHIP_OPEN} max-w-full">
        <span class="sr-only">{t('chat.footer.branchLabel')}</span>
        <GitBranch size={14} aria-hidden="true" class="shrink-0" />
        <span class="min-w-0 -my-[3px] truncate py-[3px] [text-box:trim-both_cap_alphabetic]">{branch}</span>
        {#if ahead > 0}
          <span class="tabular inline-flex shrink-0 items-center text-xs text-success"><ArrowUp size={12} aria-hidden="true" />{ahead}<span class="sr-only">{t('chat.footer.ahead', { count: ahead })}</span></span>
        {/if}
        {#if behind > 0}
          <span class="tabular inline-flex shrink-0 items-center text-xs text-warning"><ArrowDown size={12} aria-hidden="true" />{behind}<span class="sr-only">{t('chat.footer.behind', { count: behind })}</span></span>
        {/if}
        {#if sync?.main && mainNew > 0}
          <span class="tabular inline-flex shrink-0 items-center text-xs text-fg-3" aria-hidden="true">{sync.main.name}<ArrowDown size={12} class="text-warning" />{mainNew}</span>
          <span class="sr-only">{tn('branch.chip.mainNew', mainNew, { main: sync.main.name })}</span>
        {/if}
        {#if operation}
          <span class="inline-flex shrink-0 items-center gap-1 text-xs text-danger"><TriangleAlert size={12} aria-hidden="true" />{operation.kind === 'rebase' ? t('branch.chip.rebasing') : t('branch.chip.merging')}</span>
        {/if}
        <ChevronDown size={16} aria-hidden="true" class="shrink-0 @max-[460px]:hidden" />
      </button>
    {/snippet}

    {#snippet children({ close })}
      <BranchPopover {session} {close} ondrop={() => ((dropping = true), close())} />
    {/snippet}
  </Popover>
{/if}

<DropCopyDialog bind:open={dropping} />
