<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
  import { branchSync } from '$lib/stores/branch-sync.svelte';

  import Note from './Note.svelte';

  interface Props {
    sync: BranchSync;
    /** Asks the user whether to drop the copy. The question is asked outside the menu: the menu closes when focus leaves it. */
    ondrop: () => void;
  }

  let { sync, ondrop }: Props = $props();
</script>

<!-- A stash of the user's own changes is waiting (after "Stash and switch", or a rebase that could not put them back). -->
{#if sync.stashApplied}
  <!-- The changes are back in the files already: the stash holds only a copy, which the user may drop. -->
  <Note tone="success" icon={Archive} title={t('branch.stash.back')} detail={t('branch.stash.copyKept')}>
    {#snippet action()}
      <Button disabled={branchSync.busy !== null} loading={branchSync.busy === 'dropStash'} onclick={ondrop}>{t('branch.stash.drop')}</Button>
    {/snippet}
  </Note>

{:else}
  <Note tone="warning" icon={Archive} title={t('branch.stash.kept')}>
    {#snippet action()}
      <Button disabled={branchSync.busy !== null} loading={branchSync.busy === 'applyStash'} onclick={() => void branchSync.run('applyStash')}>{t('branch.stash.apply')}</Button>
    {/snippet}
  </Note>
{/if}
