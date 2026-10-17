<script lang="ts">
  import { untrack } from 'svelte';

  import Webhook from '@lucide/svelte/icons/webhook';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { IpcFailure } from '$lib/ipc/transport';
  import { project } from '$lib/stores/project.svelte';

  import { hooks } from './hooks-data.svelte';

  interface Props {
    open: boolean;
    /** How many hooks the list holds: the number the question names. */
    count: number;
    /** The hooks are gone: whatever was open on one lets go of it. */
    onremoved: () => void;
  }

  let { open = $bindable(false), count, onremoved }: Props = $props();

  // The number the question was asked with, kept while it fades out so the dialog shows the same words.
  let asked = $state(0);

  // Read when it opens only: the list shrinks while the hooks are deleted, and the question must not change under the user.
  $effect(() => {
    if (open) {
      asked = untrack(() => count);
    }
  });

  async function remove(): Promise<void> {
    try {
      await hooks.removeAll(project.active?.path ?? null);
    } catch (error) {
      throw new Error(`${t('hooks.deleteAll.failed')}: ${IpcFailure.from(error).message}`);
    }

    onremoved();
  }
</script>

<ConfirmDialog
  bind:open
  title={tn('hooks.deleteAll.title', asked)}
  description={t('hooks.deleteAll.body')}
  subject={{ icon: Webhook, name: tn('hooks.count', asked) }}
  confirmLabel={t('hooks.deleteAll.confirm')}
  cancelLabel={t('hooks.deleteAll.cancel')}
  onconfirm={remove}
/>
