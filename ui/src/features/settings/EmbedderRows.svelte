<script lang="ts">
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { EmbedderStateView, EmbedderStore } from '$lib/stores/embedder.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import type { EmbedderStatusCopy } from './embedder-status';
  import SettingRow from './SettingRow.svelte';
  import VectorSearchStatus from './VectorSearchStatus.svelte';

  interface Props {
    /** The embedder of the Harness family member the rows are for. */
    search: EmbedderStore<EmbedderStateView>;
    /** What the rows say, in the words of that member. */
    copy: EmbedderStatusCopy;
  }

  let { search, copy }: Props = $props();

  let confirmingRemoval = $state(false);

  const embedder = $derived(search.state);

  // The embedder is followed while the settings are open, and the stored vector search pref is read again with every look at them.
  $effect(() => {
    void prefs.syncServerManaged();

    return search.watch();
  });

  // A removal that is no longer offered (vector search came back on) is not asked about any more.
  $effect(() => {
    if (!search.canRemoveModel) {
      confirmingRemoval = false;
    }
  });

  async function remove(): Promise<void> {
    if (await search.removeModel()) {
      confirmingRemoval = false;
    }
  }
</script>

{#if search.vectorSupported}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <SettingRow title={t(copy.title)} description={t(copy.description)}>
      {#snippet control()}
        <Switch label={t(copy.title)} disabled={search.vectorPending} bind:checked={() => search.vectorOn, (enabled) => void search.setVectorSearch(enabled)} />
      {/snippet}
    </SettingRow>

    {#if search.vectorOn && embedder}
      <div transition:slide={{ duration: scaled(200) }}>
        <VectorSearchStatus state={embedder} {copy} />
      </div>
    {/if}
  </div>
{/if}

{#if search.canRemoveModel}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <SettingRow title={t(copy.remove.title)} description={t(copy.remove.description)}>
      {#snippet control()}
        <Button disabled={search.removing} onclick={() => (confirmingRemoval = !confirmingRemoval)}>{t(copy.remove.action)}</Button>
      {/snippet}
    </SettingRow>

    {#if confirmingRemoval}
      <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
        <div class="flex items-center justify-between gap-6 pb-4" role="alertdialog" aria-label={t(copy.remove.confirmTitle)}>
          <div class="flex min-w-0 flex-col gap-0.5">
            <p class="text-base font-medium text-fg">{t(copy.remove.confirmTitle)}</p>
            <p class="text-sm text-fg-2">{t(copy.remove.confirmBody)}</p>
            {#if search.removeFailure}<p class="text-sm text-danger" role="alert">{search.removeFailure}</p>{/if}
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <Button variant="ghost" disabled={search.removing} onclick={() => (confirmingRemoval = false)}>{t('common.cancel')}</Button>
            <Button variant="dangerQuiet" loading={search.removing} onclick={() => void remove()}>{t('common.remove')}</Button>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}
