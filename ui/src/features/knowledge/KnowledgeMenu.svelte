<script lang="ts">
  import Library from '@lucide/svelte/icons/library';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { IpcFailure } from '$lib/ipc/transport';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  // How many documents the database says it holds, whatever the table is narrowed to; null while that is not known.
  const documents = $derived(knowledgeChanges.pulse?.documents ?? null);
  const disabled = $derived(documents === null || documents === 0);

  // What the open question is about, kept after it closes so it fades out showing the same words.
  let asked = $state(0);
  let open = $state(false);

  function ask(close: () => void): void {
    close();
    asked = documents ?? 0;
    open = true;
  }

  async function remove(): Promise<void> {
    let done: { deleted: number; total: number };

    try {
      done = await knowledgeDocs.removeAll();
    } catch (error) {
      throw new Error(`${t('knowledge.deleteAll.failed')}: ${IpcFailure.from(error).message}`);
    }

    viewState.knowledgeDocId = null;
    viewState.knowledgePage = 0;

    if (done.deleted < done.total) {
      toasts.push({ title: t('knowledge.deleteAll.partial', { deleted: done.deleted, total: done.total }), tone: 'warning' });

      return;
    }

    toasts.push({ title: tn('knowledge.deleteAll.done', done.deleted), tone: 'success' });
  }
</script>

<MoreMenu label={t('knowledge.menu.label')} panelClass="w-60">
  {#snippet children({ close })}
    <MenuItem tone="danger" icon={Trash2} {disabled} onclick={() => ask(close)}>{t('knowledge.deleteAll.action')}</MenuItem>
  {/snippet}
</MoreMenu>

<ConfirmDialog
  bind:open
  title={tn('knowledge.deleteAll.title', asked)}
  description={t('knowledge.deleteAll.body')}
  subject={{ icon: Library, name: tn('knowledge.count.documents', asked) }}
  confirmLabel={t('knowledge.deleteAll.confirm')}
  cancelLabel={t('knowledge.deleteAll.cancel')}
  onconfirm={remove}
/>
