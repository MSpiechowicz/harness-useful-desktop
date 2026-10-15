<script lang="ts">
  import FileDiff from '@lucide/svelte/icons/file-diff';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
  import { IpcFailure } from '$lib/ipc/transport';
  import { git } from '$lib/stores/git.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';

  interface Props {
    /** The file to discard; set to ask, cleared when answered. */
    file: StatusEntry | null;
  }

  let { file = $bindable(null) }: Props = $props();

  // The dialog animates out after `file` is cleared, so it keeps the last file to show meanwhile.
  let shown = $state<StatusEntry | null>(null);
  $effect(() => {
    if (file) {
      shown = file;
    }
  });

  async function discard(entry: StatusEntry): Promise<void> {
    try {
      await git.discardFile(entry);
    } catch (error) {
      toasts.push({ title: t('changes.discardFile.failed'), description: IpcFailure.from(error).message, tone: 'danger' });
    }
  }
</script>

<ConfirmDialog
  bind:open={() => file !== null, (open) => { if (!open) file = null; }}
  title={t('changes.discardFile.title')}
  description={t('changes.discardFile.description')}
  subject={{ icon: FileDiff, name: shown?.path ?? '' }}
  confirmLabel={t('changes.discardFile.confirm')}
  onconfirm={() => shown && void discard(shown)}
/>
