<script lang="ts">
  import CircleDollarSign from '@lucide/svelte/icons/circle-dollar-sign';
  import Info from '@lucide/svelte/icons/info';

  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { formatShare, formatTokens, formatUsd } from '$lib/dashboard/format';
  import type { Breakdown, BreakdownRow } from '$lib/dashboard/models';
  import { dashboard, type PanelDimension, type PanelRange } from '$lib/stores/dashboard.svelte';

  import { modelColor, otherColor, providerColor } from '../charts/colors';
  import Donut from '../charts/Donut.svelte';
  import type { ShareSegment } from '../charts/share';
  import { phraseOf } from './ranges';
  import CardEmpty from './CardEmpty.svelte';
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

  /** The largest parts get their own arc; the long tail folds into one "Other" so the ring stays readable. */
  const SHOWN = 5;

  function segments(breakdown: Breakdown): ShareSegment[] {
    const ranked = [...breakdown.rows].sort((a, b) => b.costUsd - a.costUsd);
    const tail = ranked.slice(SHOWN);
    // A tail of one is shown as itself: "Other" for a single row would only hide its name.
    const shown = tail.length === 1 ? ranked : ranked.slice(0, SHOWN);
    const folded = tail.length === 1 ? [] : tail;

    // Model colors are ranked within their maker by the rows' cost order: the desktop has no all-time ranking.
    const order = ranked.map((row) => row.key);

    const parts: ShareSegment[] = shown.map((row) => ({
      key: row.key,
      label: row.key,
      value: row.costUsd,
      valueLabel: formatUsd(row.costUsd),
      shareLabel: formatShare(row.share),
      description: describe(row),
      // The same colors the Harness Dashboard gives harnesses and models.
      color: breakdown.dimension === 'provider' ? providerColor(row.key) : modelColor(row.key, order),
    }));

    if (folded.length > 0) {
      const cost = folded.reduce((sum, row) => sum + row.costUsd, 0);
      const share = folded.reduce((sum, row) => sum + row.share, 0);
      const names = folded.map((row) => row.key).join(', ');

      parts.push({
        key: '__other',
        label: 'other',
        value: cost,
        valueLabel: formatUsd(cost),
        shareLabel: formatShare(share),
        note: names,
        description: `other (${names}): ${formatUsd(cost)}, ${formatShare(share)}`,
        color: otherColor(),
      });
    }

    return parts;
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
      <CardEmpty icon={CircleDollarSign} title="Nothing was spent in this range." />
    {:else}
      <Donut segments={segments(breakdown)} label="Cost by {noun}, {phraseOf(breakdown.range, range)}" total={formatUsd(breakdown.totalCostUsd)} caption="total" />

      <p class="tabular mt-3 flex items-center gap-1.5 border-t border-line pt-3 text-xs text-fg-3">
        <Info size={16} class="shrink-0" aria-hidden="true" />
        Total {formatUsd(breakdown.totalCostUsd)} · {formatTokens(breakdown.totalTokens)} tokens, at API list prices.
        {#if breakdown.rows.some((row) => row.estimatedPrice)}Some prices are estimated.{/if}
      </p>
    {/if}
  {/snippet}
</SlotCard>
