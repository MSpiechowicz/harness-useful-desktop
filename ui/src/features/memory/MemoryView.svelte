<script lang="ts">
  import Brain from '@lucide/svelte/icons/brain';
  import FileCode from '@lucide/svelte/icons/file-code';
  import NotebookText from '@lucide/svelte/icons/notebook-text';

  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { type MemorySection, viewState } from '$lib/stores/view-state.svelte';

  import DeleteAllMenu from './DeleteAllMenu.svelte';
  import { MEMORY_HEADER_ACTIONS } from './HeaderActions.svelte';
  import { useMemoryData } from './memory-data.svelte';
  import NotesSection from './NotesSection.svelte';
  import ScriptsSection from './ScriptsSection.svelte';

  useMemoryData();

  const sections = $derived<{ value: MemorySection; label: string; icon: IconComponent }[]>([
    { value: 'notes', label: t('memory.section.notes'), icon: NotebookText },
    { value: 'scripts', label: t('memory.section.scripts'), icon: FileCode },
  ]);
</script>

<div class="flex size-full min-h-0 flex-col overflow-clip">
  <ViewHeader title={t('memory.title')} icon={Brain}>
    {#snippet actions()}
      <div id={MEMORY_HEADER_ACTIONS} class="contents"></div>
      <FilterMenu label={t('memory.section.label')} options={sections} value={viewState.memorySection} onchange={(next) => (viewState.memorySection = next)} />
      <DeleteAllMenu section={viewState.memorySection} />
    {/snippet}
  </ViewHeader>

  {#if viewState.memorySection === 'notes'}
    <NotesSection />
  {:else}
    <ScriptsSection />
  {/if}
</div>
