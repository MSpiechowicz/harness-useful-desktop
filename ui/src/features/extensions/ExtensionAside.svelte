<script lang="ts">
  import FactsCard from '$lib/components/FactsCard.svelte';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import StatGrid from '$lib/components/StatGrid.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { detailBasics, overviewStats } from './extension-detail';
  import type { ExtItem } from './extension-items';

  interface Props {
    entry: ExtItem;
  }

  let { entry }: Props = $props();

  const stats = $derived(overviewStats(entry));
  const details = $derived(detailBasics(entry).facts.filter((fact) => !fact.location));
</script>

<!-- The body of the details panel: the item's numbers and facts. The panel brings the padding, the gap and the scrolling. -->
{#if stats.length > 0}
  <SectionCard flat heading={t('extensions.overview.title')}>
    <StatGrid {stats} />
  </SectionCard>
{/if}

<FactsCard flat heading={t('extensions.detail.details')} rows={details} />
