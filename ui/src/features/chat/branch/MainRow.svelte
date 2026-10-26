<script lang="ts">
  import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import GitMerge from '@lucide/svelte/icons/git-merge';
  import GitPullRequestArrow from '@lucide/svelte/icons/git-pull-request-arrow';

  import Button from '$lib/components/Button.svelte';
  import SplitButton from '$lib/components/SplitButton.svelte';
  import type { SplitItem } from '$lib/components/split';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
  import type { MainBranch } from '$lib/ipc/bindings/MainBranch';
  import { branchSync } from '$lib/stores/branch-sync.svelte';

  import SyncRow from './SyncRow.svelte';

  interface Props {
    sync: BranchSync;
    /** The main branch, or null when none was found. */
    main: MainBranch | null;
    /** Opens the picker where the user chooses the main branch. */
    onpick: () => void;
  }

  let { sync, main, onpick }: Props = $props();

  const busy = $derived(branchSync.busy !== null);

  const items = $derived.by((): SplitItem[] => {
    if (!main) {
      return [];
    }

    const entries: SplitItem[] = [
      { id: 'rebase', label: t('branch.main.rebase', { main: main.name }), description: t('branch.main.rebaseHint'), icon: GitPullRequestArrow, onselect: () => void branchSync.run('rebase') },
      { id: 'merge', label: t('branch.main.merge', { main: main.name }), description: t('branch.main.mergeHint'), icon: GitMerge, onselect: () => void branchSync.run('merge') },
    ];

    if (sync.upstream !== null) {
      entries.push({ id: 'pull', label: t('branch.main.pull', { branch: sync.branch ?? '' }), description: t('branch.main.pullHint'), icon: ArrowDownToLine, onselect: () => void branchSync.run('pull') });
    }

    return entries;
  });

  const subtitle = $derived(main ? (main.newCommits > 0 ? tn('branch.main.new', main.newCommits) : t('branch.main.upToDate')) : t('branch.main.none'));
</script>

<SyncRow icon={GitMerge} {subtitle}>
  {#snippet title()}
    {#if main}
      <!-- The name opens the picker: the guess can be wrong, and the choice is kept for this project. -->
      <button
        type="button"
        title={t('branch.main.change')}
        aria-label={t('branch.main.change') + ': ' + main.name}
        disabled={busy}
        class="-mx-1.5 inline-flex max-w-full items-center gap-1 h-5 rounded-sm px-1.5 hover:bg-hover focus-visible:bg-hover disabled:cursor-not-allowed disabled:hover:bg-transparent"
        onclick={onpick}
      >
        <span class="truncate">{main.name}</span>
        <ChevronsUpDown size={14} aria-hidden="true" class="shrink-0 text-fg-3" />
      </button>
    {:else}
      {t('branch.main.title')}
    {/if}
  {/snippet}

  {#snippet action()}
    {#if main}
      <SplitButton
        variant={main.newCommits > 0 ? 'primary' : 'secondary'}
        icon={ArrowDownToLine}
        label={t('branch.main.rebase', { main: main.name })}
        menuLabel={t('branch.main.more')}
        disabled={busy || main.newCommits === 0}
        loading={branchSync.busy === 'rebase' || branchSync.busy === 'merge'}
        {items}
        onclick={() => void branchSync.run('rebase')}
      />
    {:else}
      <Button disabled={busy} onclick={onpick}>{t('branch.main.choose')}</Button>
    {/if}
  {/snippet}
</SyncRow>
