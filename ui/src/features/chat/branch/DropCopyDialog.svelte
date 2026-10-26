<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { branchSync } from '$lib/stores/branch-sync.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';

  interface Props {
    open?: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  // The menu is closed by now, so the outcome is a toast (a failure already has its own).
  async function drop(): Promise<void> {
    await branchSync.run('dropStash');

    if (branchSync.state?.stashKept === false) {
      toasts.push({ title: t('branch.note.stashDropped'), tone: 'success' });
    }
  }
</script>

<ConfirmDialog bind:open title={t('branch.stash.dropTitle')} description={t('branch.stash.dropDescription')} subject={{ icon: Archive, name: t('branch.stash.copyKept') }} confirmLabel={t('branch.stash.drop')} onconfirm={() => void drop()} />
