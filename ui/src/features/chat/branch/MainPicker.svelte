<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import GitMerge from '@lucide/svelte/icons/git-merge';

  import Button from '$lib/components/Button.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
  import { branchSync } from '$lib/stores/branch-sync.svelte';

  import BranchRows from './BranchRows.svelte';
  import SyncRow from './SyncRow.svelte';

  interface Props {
    sync: BranchSync;
    onback: () => void;
  }

  let { sync, onback }: Props = $props();

  let query = $state('');

  const main = $derived(sync.main);
  const busy = $derived(branchSync.busy !== null);

  $effect(() => {
    branchSync.search(query, query === '');
  });

  // Saving the pick takes a moment: the panel goes back once the new main branch is known.
  async function pick(branch: string | null): Promise<void> {
    await branchSync.pickMain(branch);
    onback();
  }
</script>

<!-- A sub-view of the branch menu, not a menu of its own: a popover inside a popover would be clipped by it. -->
<div class="flex items-center gap-1 border-b border-line px-1.5 py-1.5">
  <IconButton icon={ArrowLeft} label={t('branch.picker.back')} onclick={onback} />
  <span class="text-sm font-semibold">{t('branch.main.title')}</span>
</div>

{#if main}
  <div class="mx-2.5 my-2.5 rounded-md border border-line bg-surface-2">
    <SyncRow icon={GitMerge} subtitle={t(`branch.picker.source.${main.source}`, { remote: sync.remote ?? '' })}>
      {#snippet title()}<span class="font-mono">{main.name}</span>{/snippet}

      {#snippet action()}
        {#if main.source === 'picked'}
          <Button disabled={busy} loading={branchSync.busy === 'main'} onclick={() => void pick(null)}>{t('branch.picker.useDetected')}</Button>
        {/if}
      {/snippet}
    </SyncRow>
  </div>
{/if}

<div class="px-1.5 pb-1.5" role="presentation" onkeydown={(event) => (event.key === 'Home' || event.key === 'End') && event.stopPropagation()}>
  <SearchField bind:value={query} label={t('branch.picker.search')} placeholder={t('branch.picker.search')} autofocus />
</div>

<BranchRows remote={sync.remote} selected={(row) => row.name === main?.name} disabled={busy} onpick={(row) => void pick(row.name)} />
