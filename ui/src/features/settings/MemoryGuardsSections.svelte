<script lang="ts">
  import { untrack } from 'svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { cx } from '$lib/utils/cx';

  import MemoryGuardsCard from './MemoryGuardsCard.svelte';

  /** Without Project memory no harness runs the server, so the cards keep their settings but are greyed out. */
  const memoryOn = $derived(prefs.value?.memoryEnabled !== false);

  // Read when the cards are first needed, and whenever Harness Memory turns out to support them.
  $effect(() => {
    if (memoryChat.supported) {
      untrack(() => void memoryChat.loadGlobal());
    }
  });
</script>

<!-- What agents may save, per kind: the Notes and Scripts cards under Project memory. They are absent for a Harness Memory that cannot apply them. -->
{#if memoryChat.supported}
  {#if memoryChat.global !== null}
    <div class={cx('mt-5 flex flex-col gap-8 transition-opacity duration-[var(--dur-fast)]', !memoryOn && 'opacity-50')} inert={!memoryOn}>
      <MemoryGuardsCard kind="notes" />
      <MemoryGuardsCard kind="scripts" />
    </div>
  {:else if memoryChat.globalFailed}
    <p class="mt-5 px-1 text-sm text-danger" role="alert">{t('memory.guards.settings.loadFailed')}</p>
  {/if}
{/if}
