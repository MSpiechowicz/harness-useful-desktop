<script lang="ts">
  import Info from '@lucide/svelte/icons/info';

  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { formatShare, formatTokens, formatUsd } from '$lib/dashboard/format';
  import type { Breakdown, BreakdownRow } from '$lib/dashboard/models';
  import { dashboard, type PanelDimension, type PanelRange } from '$lib/stores/dashboard.svelte';

  import BarList from '../charts/BarList.svelte';
  import type { BarDatum } from '../charts/bars';
  import { ACCENT_COLOR, providerColor } from '../charts/colors';
  import { phraseOf } from './ranges';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    range: PanelRange;
    index?: number;
  }

  let { range, index = 0 }: Props = $props();

  const DIMENSIONS: StripOption<PanelDimension>[] = [
    { value: 'model', label: 'By model' },
    { value: 'provider', label: 'By harness' },
  ];

  const slot = $derived(dashboard.dimension === 'model' ? dashboard.byModel : dashboard.byProvider);
  const phrase = $derived(phraseOf(slot.data?.range, range));
  const noun = $derived(dashboard.dimension === 'model' ? 'model' : 'harness');

  function describe(row: BreakdownRow): string {
    const parts = [`${row.key}: ${formatUsd(row.costUsd)}`, formatShare(row.share), `${formatTokens(row.tokens)} tokens`, `${row.sessions} sessions`];
    if (row.estimatedPrice) {
      parts.push('price estimated');
    }

    return parts.join(', ');
  }

  function bars(breakdown: Breakdown): BarDatum[] {
    return breakdown.rows.map((row) => ({
      key: row.key,
      label: row.key,
      value: row.costUsd,
      valueLabel: `${formatUsd(row.costUsd)} · ${formatShare(row.share)}`,
      description: describe(row),
      // Only harnesses carry a color of their own; models are ranked by length alone.
      color: breakdown.dimension === 'provider' ? providerColor(row.key) : ACCENT_COLOR,
    }));
  }

  function load(): void {
    void dashboard.openPanel(true);
  }
</script>

<SlotCard title="Where the spend goes" subtitle="Cost by {noun} for {phrase}" {slot} onretry={load} {index} class="@container">
  {#snippet actions()}
    <SegmentedControl label="Group the cost by" options={DIMENSIONS} bind:value={() => dashboard.dimension, (next) => (dashboard.dimension = next)} />
  {/snippet}

  {#snippet children(breakdown)}
    {#if breakdown.rows.length === 0}
      <p class="py-4 text-sm text-fg-3" role="status">Nothing was spent in this range.</p>
    {:else}
      <BarList data={bars(breakdown)} label="Cost by {noun}, {phraseOf(breakdown.range, range)}" />

      <p class="tabular mt-3 flex items-center gap-1.5 border-t border-line pt-3 text-xs text-fg-3">
        <Info size={16} class="shrink-0" aria-hidden="true" />
        Total {formatUsd(breakdown.totalCostUsd)} · {formatTokens(breakdown.totalTokens)} tokens, at API list prices.
        {#if breakdown.rows.some((row) => row.estimatedPrice)}Some prices are estimated.{/if}
      </p>
    {/if}
  {/snippet}
</SlotCard>
