<script lang="ts">
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { memorySearch } from '$lib/stores/memoryEmbedder.svelte';

  import SettingRow from './SettingRow.svelte';
  import VectorSearchStatus from './VectorSearchStatus.svelte';

  let confirmingRemoval = $state(false);

  const embedder = $derived(memorySearch.state);
  // The English notes setting is team-wide on a team database: its description says so only then.
  const teamDatabase = $derived(memoryRemote.value?.enabled === true);
  const englishDescription = $derived(t(teamDatabase ? 'settings.memory.english.descriptionTeam' : 'settings.memory.english.description'));

  // The embedder is followed while the settings are open, and the settings are read again with every look at them.
  $effect(() => {
    void memorySearch.loadSettings();

    return memorySearch.watch();
  });

  // A removal that is no longer offered (vector search came back on) is not asked about any more.
  $effect(() => {
    if (!memorySearch.canRemoveModel) {
      confirmingRemoval = false;
    }
  });

  async function remove(): Promise<void> {
    if (await memorySearch.removeModel()) {
      confirmingRemoval = false;
    }
  }
</script>

{#if memorySearch.vectorSupported}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <SettingRow title={t('settings.memory.vector.title')} description={t('settings.memory.vector.description')}>
      {#snippet control()}
        <Switch label={t('settings.memory.vector.title')} disabled={memorySearch.vectorPending} bind:checked={() => memorySearch.vectorOn, (enabled) => void memorySearch.setVectorSearch(enabled)} />
      {/snippet}
    </SettingRow>

    {#if memorySearch.vectorOn && embedder}
      <div transition:slide={{ duration: scaled(200) }}>
        <VectorSearchStatus state={embedder} />
      </div>
    {/if}
  </div>
{/if}

{#if memorySearch.canRemoveModel}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <SettingRow title={t('settings.memory.vector.remove.title')} description={t('settings.memory.vector.remove.description')}>
      {#snippet control()}
        <Button disabled={memorySearch.removing} onclick={() => (confirmingRemoval = !confirmingRemoval)}>{t('settings.memory.vector.remove.action')}</Button>
      {/snippet}
    </SettingRow>

    {#if confirmingRemoval}
      <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
        <div class="flex items-center justify-between gap-6 pb-4" role="alertdialog" aria-label={t('settings.memory.vector.remove.confirmTitle')}>
          <div class="flex min-w-0 flex-col gap-0.5">
            <p class="text-base font-medium text-fg">{t('settings.memory.vector.remove.confirmTitle')}</p>
            <p class="text-sm text-fg-2">{t('settings.memory.vector.remove.confirmBody')}</p>
            {#if memorySearch.removeFailure}<p class="text-sm text-danger" role="alert">{memorySearch.removeFailure}</p>{/if}
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <Button variant="ghost" disabled={memorySearch.removing} onclick={() => (confirmingRemoval = false)}>{t('common.cancel')}</Button>
            <Button variant="dangerQuiet" loading={memorySearch.removing} onclick={() => void remove()}>{t('common.remove')}</Button>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}

{#if memorySearch.settingsSupported && memorySearch.englishNotes !== null}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <SettingRow title={t('settings.memory.english.title')} description={englishDescription}>
      {#snippet control()}
        <Switch label={t('settings.memory.english.title')} disabled={memorySearch.englishPending} bind:checked={() => memorySearch.englishNotes ?? true, (on) => void memorySearch.setEnglishNotes(on)} />
      {/snippet}
    </SettingRow>
  </div>
{/if}
