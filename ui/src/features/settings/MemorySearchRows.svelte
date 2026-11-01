<script lang="ts">
  import { slide } from 'svelte/transition';

  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { memorySearch } from '$lib/stores/memoryEmbedder.svelte';

  import { MEMORY_EMBEDDER_COPY } from './embedder-status';
  import EmbedderRows from './EmbedderRows.svelte';
  import SettingRow from './SettingRow.svelte';

  // The English notes setting is team-wide on a team database: its description says so only then.
  const teamDatabase = $derived(memoryRemote.value?.enabled === true);
  const englishDescription = $derived(t(teamDatabase ? 'settings.memory.english.descriptionTeam' : 'settings.memory.english.description'));

  // The settings are read again with every look at them.
  $effect(() => {
    void memorySearch.loadSettings();
  });
</script>

<EmbedderRows search={memorySearch} copy={MEMORY_EMBEDDER_COPY} />

{#if memorySearch.settingsSupported && memorySearch.englishNotes !== null}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <SettingRow title={t('settings.memory.english.title')} description={englishDescription}>
      {#snippet control()}
        <Switch label={t('settings.memory.english.title')} disabled={memorySearch.englishPending} bind:checked={() => memorySearch.englishNotes ?? true, (on) => void memorySearch.setEnglishNotes(on)} />
      {/snippet}
    </SettingRow>
  </div>
{/if}
