<script lang="ts">
  import FileUp from '@lucide/svelte/icons/file-up';

  import MenuItem from '$lib/components/MenuItem.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { noteList } from '$lib/stores/memoryNotes.svelte';

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

  useNotesData();

  let copying = $state(false);

  const headerActions = provideHeaderActions('notes-header-actions');
  // There is nothing to save to while Harness Memory is missing, too old or not answering.
  const available = $derived(noteList.data?.availability.kind === 'available' && noteList.status !== 'error');

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
  <MemoryHeader view="notes" slot={headerActions} addLabel={t('memory.editor.add.note')} {available} onadd={() => noteEditor.create()} extra={importItem} />

  <MemoryOfflineBanner page={noteList.data} />

  <NotesCopyBanner oncopy={() => (copying = true)} />

  <NotesSection />
</div>

<NotesImportDialog />
<NotesCopyDialog open={copying} onclose={() => (copying = false)} />
