<script lang="ts">
  import Spinner from '$lib/components/Spinner.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeImport as flow } from '$lib/stores/knowledgeImport.svelte';

  const label = $derived(flow.run.total === null ? t('knowledge.import.chip.reading') : t('knowledge.import.chip', { done: flow.run.done, total: flow.run.total }));
</script>

<!-- The import goes on with the window closed: this is where its progress stays, and the way back into it. -->
{#if flow.running && !flow.open}
  <button
    type="button"
    in:rise={{ y: 4, duration: 200 }}
    aria-label={t('knowledge.import.chip.open')}
    class="inline-flex h-control items-center gap-2 rounded-md border border-accent/30 bg-accent-wash px-3 text-sm font-medium whitespace-nowrap text-accent-text transition-[transform,background-color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] hover:bg-accent-wash/70 active:scale-[0.97]"
    onclick={() => flow.begin()}
  >
    <Spinner size={16} />
    <span class="tabular">{label}</span>
  </button>
{/if}
