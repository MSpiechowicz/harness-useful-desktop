<script lang="ts">
  import { untrack } from 'svelte';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import { ICON } from '$lib/components/icon';
  import LoadingSpinner from '$lib/components/LoadingSpinner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { cx } from '$lib/utils/cx';

  import MemoryGuardsCard from './MemoryGuardsCard.svelte';
  import UpdateNotice from './UpdateNotice.svelte';

  /** Without Project memory no harness runs the server, so the cards keep their settings but are greyed out. */
  const memoryOn = $derived(prefs.value?.memoryEnabled !== false);

  // An installed Harness Memory that predates where-to-save (no guards, or guards without the opt-in) is updated right here. A missing one has nothing to update.
  const memoryInstalled = $derived(ecosystem.status?.items.find((item) => item.id === 'memory')?.installed === true);
  const outdated = $derived(memoryInstalled && memoryRemote.value !== null && !memoryChat.supported);

  // Read when the cards are first needed, and whenever Harness Memory turns out to support them. Read again by itself while a read failed or the old file is in use.
  $effect(() => {
    if (memoryChat.supported) {
      return untrack(() => memoryChat.watchGlobal());
    }
  });

  // Whether Harness Memory is installed is known from the ecosystem reading, which the shell keeps current.
  $effect(() => untrack(() => void ecosystem.load()));
</script>

<!-- What agents may save, per kind: the Notes and Scripts cards under Project memory. An older Harness Memory cannot apply them, so a row offers its update instead. -->
{#if memoryChat.supported}
  {#if memoryChat.global === null}
    {#if memoryChat.globalFailed}
      <p class="mt-5 px-1 text-sm text-danger" role="alert">{t('memory.guards.settings.loadFailed')}</p>
    {:else}
      <LoadingSpinner label={t('memory.guards.settings.loading')} class="mt-5 py-10" />
    {/if}
  {:else}
    <div class={cx('mt-5 flex flex-col gap-8 transition-opacity duration-[var(--dur-fast)]', !memoryOn && 'opacity-50')} inert={!memoryOn}>
      {#if memoryChat.globalFailed}
        <!-- A read after the first failed: the cards stay as they were last read (an open dialog stays too), and the alert says they may be out of date. -->
        <p class="px-1 text-sm text-danger" role="alert">{t('memory.guards.settings.loadFailed')}</p>
      {/if}

      {#if memoryChat.global.migrationPending}
        <!-- One line for both cards: the file is shared, so the warning is too. -->
        <div class="flex items-center gap-3 rounded-md bg-warning-wash px-3.5 py-2 text-sm text-warning" role="status">
          <TriangleAlert size={ICON.sm} aria-hidden="true" class="shrink-0" />
          {t('memory.guards.settings.migrationPending')}
        </div>
      {/if}

      <MemoryGuardsCard kind="notes" />
      <MemoryGuardsCard kind="scripts" />
    </div>
  {/if}
{:else if outdated}
  <UpdateNotice class="mt-5" text={t('memory.guards.settings.update')} onfinished={() => memoryRemote.load()} />
{/if}
