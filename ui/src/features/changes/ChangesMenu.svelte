<script lang="ts">
  import FileDiff from '@lucide/svelte/icons/file-diff';
  import FileX from '@lucide/svelte/icons/file-x';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import { entrySections, SCOPE_LABELS, SECTIONS } from '$lib/git/entries';
  import { normalizeQuery } from '$lib/git/search';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
  import { IpcFailure } from '$lib/ipc/transport';
  import { git } from '$lib/stores/git.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';

  interface Props {
    /** Name of the open project, for the confirmation. */
    project: string;
    /** The file in view, if any: the menu then offers to discard just it. */
    file: StatusEntry | undefined;
    /** Asks to discard `file` (the confirmation lives with the view, the header button asks it too). */
    ondiscardfile: (file: StatusEntry) => void;
  }

  let { project, file, ondiscardfile }: Props = $props();

  let confirmOpen = $state(false);

  /** Every changed file, whatever the filter or the search hides: discarding takes all of them. */
  const entries = $derived(git.status?.entries ?? []);
  const perSection = $derived(
    SECTIONS.map((section) => ({ section, files: entries.filter((entry) => entrySections(entry).includes(section)).length }))
      .filter((group) => group.files > 0)
      .map((group) => `${SCOPE_LABELS[group.section]} ${group.files}`)
      .join(' · '),
  );
  const hidden = $derived(git.filter !== null || normalizeQuery(git.search) !== null);

  function ask(close: () => void): void {
    close();
    confirmOpen = true;
  }

  function askFile(close: () => void, entry: StatusEntry): void {
    close();
    ondiscardfile(entry);
  }

  async function discard(): Promise<void> {
    try {
      await git.discardAll();
    } catch (error) {
      toasts.push({ title: t('changes.discardAll.failed'), description: IpcFailure.from(error).message, tone: 'danger' });
    }
  }
</script>

<MoreMenu label={t('changes.menu.label')} panelClass="w-56">
  {#snippet children({ close })}
    {#if file}
      {@const entry = file}
      <MenuItem tone="danger" icon={FileX} onclick={() => askFile(close, entry)}>{t('changes.discardFile.action')}</MenuItem>
    {/if}
    <MenuItem tone="danger" icon={Trash2} onclick={() => ask(close)}>{t('changes.discardAll.action')}</MenuItem>
  {/snippet}
</MoreMenu>

<ConfirmDialog
  bind:open={confirmOpen}
  title={t('changes.discardAll.title')}
  description={t('changes.discardAll.description', { project })}
  subject={{ icon: FileDiff, name: tn('changes.discardAll.files', entries.length), details: perSection, note: hidden ? t('changes.discardAll.hidden') : undefined }}
  confirmLabel={t('changes.discardAll.confirm')}
  onconfirm={() => void discard()}
/>
