<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { scriptList } from '$lib/stores/memoryScripts.svelte';

  import { scriptEditor } from './editor-state.svelte';
  import { provideHeaderActions } from './HeaderActions.svelte';
  import { useScriptsData } from './memory-data.svelte';
  import MemoryHeader from './MemoryHeader.svelte';
  import ScriptsSection from './ScriptsSection.svelte';

  useScriptsData();

  const headerActions = provideHeaderActions('scripts-header-actions');
  // There is nothing to save to while Harness Memory is missing, too old or not answering.
  const available = $derived(scriptList.data?.availability.kind === 'available' && scriptList.status !== 'error');
</script>

<div class="flex size-full min-h-0 flex-col overflow-clip">
  <MemoryHeader view="scripts" slot={headerActions} addLabel={t('memory.editor.add.script')} {available} onadd={() => scriptEditor.create()} />

  <ScriptsSection />
</div>
