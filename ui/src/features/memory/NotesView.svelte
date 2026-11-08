<script lang="ts">
  import FileUp from '@lucide/svelte/icons/file-up';

  import MenuItem from '$lib/components/MenuItem.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { noteList } from '$lib/stores/memoryNotes.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import { CopyDialogState } from './copy-dialog.svelte';
  import { noteEditor } from './editor-state.svelte';
  import { provideHeaderActions } from './HeaderActions.svelte';
  import { useNotesData } from './memory-data.svelte';
  import MemoryHeader from './MemoryHeader.svelte';
  import MemoryOfflineBanner from './MemoryOfflineBanner.svelte';
  import NotesCopyBanner from './NotesCopyBanner.svelte';
  import NotesCopyDialog from './NotesCopyDialog.svelte';
  import { notesImport } from './notes-import-state.svelte';
  import NotesImportDialog from './NotesImportDialog.svelte';
  import NotesSection from './NotesSection.svelte';
  import { syncDirections } from './sync-offer';

  useNotesData();

  const dialog = new CopyDialogState();

  const headerActions = provideHeaderActions('notes-header-actions');
  // There is nothing to save to while Harness Memory is missing, too old or not answering.
  const available = $derived(noteList.data?.availability.kind === 'available' && noteList.status !== 'error');
  const directions = $derived(syncDirections('notes', memoryRemote.value, noteList.data, prefs.value));

  function startImport(close: () => void): void {
    close();
    notesImport.begin();
  }
</script>

<!-- Notes are only imported where they can be saved, so the item follows the Add button. -->
{#snippet importItem({ close }: { close: () => void })}
  {#if available}
    <MenuItem icon={FileUp} disabled={notesImport.picking} onclick={() => startImport(close)}>{t('memory.import.menu')}</MenuItem>
  {/if}
{/snippet}

<div class="flex size-full min-h-0 flex-col overflow-clip">
  <MemoryHeader view="notes" slot={headerActions} addLabel={t('memory.editor.add.note')} {available} onadd={() => noteEditor.create()} onsync={directions.length > 0 ? () => (dialog.syncing = directions) : undefined} extra={importItem} />

  <MemoryOfflineBanner page={noteList.data} />

  <NotesCopyBanner oncopy={() => (dialog.copying = true)} />

  <NotesSection />
</div>

<NotesImportDialog />
<NotesCopyDialog open={dialog.open} directions={dialog.syncing ?? undefined} onclose={() => dialog.close()} />
