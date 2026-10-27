<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';

  import Button from '$lib/components/Button.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import { isBranchName, slugBranchName } from '$lib/git/branch-name';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { BranchStart } from '$lib/ipc/bindings/BranchStart';
  import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
  import { branchSync } from '$lib/stores/branch-sync.svelte';

  import BranchRows from './BranchRows.svelte';
  import SectionLabel from './SectionLabel.svelte';

  interface Props {
    /** Null while the repository could not be read: the branches can still be listed and switched. */
    sync: BranchSync | null;
  }

  let { sync }: Props = $props();

  let query = $state('');
  /** "New branch" was pressed: the field names a branch instead of only searching. */
  let creating = $state(false);
  let from = $state<BranchStart>('main');

  // A stopped rebase or merge has to end before the branch can change.
  // Conflict markers left by a stash (no operation) block a switch as well.
  const locked = $derived(branchSync.busy !== null || sync?.operation != null || (sync?.unmerged.length ?? 0) > 0);
  const slug = $derived(slugBranchName(query));
  const exists = $derived(branchSync.page?.branches.some((row) => row.name === slug) ?? false);
  // Only after "New branch": a search for a name that does not exist offers nothing to create.
  const creatable = $derived(creating && isBranchName(slug) && !exists);
  const carried = $derived((sync?.trackedChanges ?? 0) + (sync?.untracked ?? 0));
  const starts = $derived([
    ...(sync?.main ? [{ value: 'main' as const, label: sync.main.name }] : []),
    { value: 'current' as const, label: t('branch.create.fromCurrent') },
  ]);

  $effect(() => {
    branchSync.search(query, query === '');
  });

  // Without a main branch a new branch can only start from the current one.
  $effect(() => {
    if (!sync?.main && from === 'main') {
      from = 'current';
    }
  });

  function create(): void {
    if (!creatable || locked) {
      return;
    }

    void branchSync.switchTo({ branch: slug, create: from, stash: false });
    query = '';
    creating = false;
  }

  function switchTo(branch: string): void {
    query = '';
    void branchSync.switchTo({ branch, create: null, stash: false });
  }

  function onSearchKey(event: KeyboardEvent): void {
    if (event.key === 'Enter' && creatable) {
      event.preventDefault();
      create();
    }
  }
</script>

<div class="pt-1">
  <SectionLabel>{creating ? t('branch.list.new') : t('branch.list.title')}</SectionLabel>

  <div class="flex gap-2 px-2.5 pb-1.5">
    <SearchField class="min-w-0 flex-1" bind:value={query} label={t('branch.list.searchLabel')} placeholder={creating ? t('branch.list.namePlaceholder') : t('branch.list.searchPlaceholder')} onkeydown={onSearchKey} autofocus />

    {#if creating}
      <Button class="shrink-0" onclick={() => ((creating = false), (query = ''))}>{t('branch.list.cancel')}</Button>
    {:else}
      <Button class="shrink-0" disabled={locked} onclick={() => (creating = true)}>
        {#snippet leading()}<Plus size={16} aria-hidden="true" />{/snippet}
        {t('branch.list.new')}
      </Button>
    {/if}
  </div>
</div>

{#if creatable || creating}
  <div class="mb-1 flex flex-col gap-1.5 border-b border-line px-2.5 pb-2">
    {#if creatable}
      <button
        type="button"
        data-menu-item
        disabled={locked}
        class="flex h-control w-full items-center gap-2 rounded-md bg-accent-wash px-2 text-left text-sm text-accent-text transition-[filter] duration-[var(--dur-fast)] hover:brightness-125 disabled:cursor-not-allowed disabled:opacity-60"
        onclick={create}
      >
        <Plus size={16} aria-hidden="true" class="shrink-0" />
        <span class="min-w-0 truncate">{t('branch.create.row', { name: slug })}</span>
        <kbd class="ml-auto shrink-0 rounded-sm border border-line-strong px-1.5 font-mono text-xs text-fg-3">Enter</kbd>
      </button>
    {/if}

    {#if starts.length > 1}
      <div class="flex items-center gap-2 px-1 text-xs text-fg-3">
        <span>{t('branch.create.from')}</span>
        <SegmentedControl options={starts} bind:value={from} label={t('branch.create.fromLabel')} />
      </div>
    {/if}

    {#if creatable && carried > 0}
      <p class="px-1 text-xs text-fg-3">{tn('branch.create.carry', carried)}</p>
    {/if}
  </div>
{/if}

<BranchRows remote={sync?.remote ?? null} selected={(row) => row.isCurrent} disabled={locked} showEmpty={!creatable} onpick={(row) => !row.isCurrent && switchTo(row.name)} />
