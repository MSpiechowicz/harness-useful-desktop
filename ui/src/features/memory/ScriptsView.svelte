<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { scriptList } from '$lib/stores/memoryScripts.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import { CopyDialogState } from './copy-dialog.svelte';
  import { scriptEditor } from './editor-state.svelte';
  import { provideHeaderActions } from './HeaderActions.svelte';
  import { useScriptsData } from './memory-data.svelte';
  import MemoryHeader from './MemoryHeader.svelte';
  import MemoryOfflineBanner from './MemoryOfflineBanner.svelte';
  import NotesCopyBanner from './NotesCopyBanner.svelte';
  import NotesCopyDialog from './NotesCopyDialog.svelte';
  import ScriptsSection from './ScriptsSection.svelte';
  import { syncDirections } from './sync-offer';

  useScriptsData();

  const dialog = new CopyDialogState();

  const headerActions = provideHeaderActions('scripts-header-actions');
  // There is nothing to save to while Harness Memory is missing, too old or not answering.
  const available = $derived(scriptList.data?.availability.kind === 'available' && scriptList.status !== 'error');
  const directions = $derived(syncDirections('scripts', memoryRemote.value, scriptList.data, prefs.value));
</script>

<div class="flex size-full min-h-0 flex-col overflow-clip">
  <MemoryHeader view="scripts" slot={headerActions} addLabel={t('memory.editor.add.script')} {available} onadd={() => scriptEditor.create()} onsync={directions.length > 0 ? () => (dialog.syncing = directions) : undefined} />

  <MemoryOfflineBanner page={scriptList.data} />

  <NotesCopyBanner kind="scripts" oncopy={() => (dialog.copying = true)} />

  <ScriptsSection />
</div>

<NotesCopyDialog kind="scripts" open={dialog.open} directions={dialog.syncing ?? undefined} onclose={() => dialog.close()} />
