<script lang="ts">
  import CircleDollarSign from '@lucide/svelte/icons/circle-dollar-sign';
  import Info from '@lucide/svelte/icons/info';

  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { formatShare, formatTokens, formatUsd } from '$lib/dashboard/format';
  import type { Breakdown, BreakdownRow } from '$lib/dashboard/models';
  import { t, tn } from '$lib/i18n/i18n.svelte';
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

  const DIMENSIONS = $derived<StripOption<PanelDimension>[]>([
    { value: 'model', label: t('ecosystem.breakdown.byModel') },
    { value: 'provider', label: t('ecosystem.breakdown.byHarness') },
  ]);

  const slot = $derived(dashboard.dimension === 'model' ? dashboard.byModel : dashboard.byProvider);
  const phrase = $derived(phraseOf(slot.data?.range, range));
  const by = $derived(dashboard.dimension === 'model' ? 'model' : 'provider');

  /** "17.6k tokens": the compact figure, with the noun agreeing with the exact count. */
  function tokensText(count: number): string {
    return tn('ecosystem.count.token', count, { tokens: formatTokens(count) });
  }

  function describe(row: BreakdownRow): string {
    const parts = [
      `${row.key}: ${formatUsd(row.costUsd)}`,
      formatShare(row.share),
      tokensText(row.tokens),
      tn('ecosystem.count.session', row.sessions),
    ];
    if (row.estimatedPrice) {
      parts.push(t('ecosystem.breakdown.priceEstimated'));
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
        label: t('ecosystem.breakdown.rest'),
        value: cost,
        valueLabel: formatUsd(cost),
        shareLabel: formatShare(share),
        note: names,
        description: t('ecosystem.breakdown.otherDescription', { names, cost: formatUsd(cost), share: formatShare(share) }),
        color: otherColor(),
      });
    }

    return parts;
  }

  function load(): void {
    void dashboard.openPanel(true);
  }
</script>

<SlotCard title={t('ecosystem.breakdown.title')} subtitle={t(`ecosystem.breakdown.subtitle.${by}`, { phrase })} {slot} onretry={load} {index} class="@container">
  {#snippet actions()}
    <SegmentedControl label={t('ecosystem.breakdown.group')} options={DIMENSIONS} bind:value={() => dashboard.dimension, (next) => (dashboard.dimension = next)} />
  {/snippet}

  {#snippet children(breakdown)}
    {#if breakdown.rows.length === 0}
      <CardEmpty icon={CircleDollarSign} title={t('ecosystem.breakdown.empty')} />
    {:else}
      <Donut
        segments={segments(breakdown)}
        label={t(`ecosystem.breakdown.label.${by}`, { phrase: phraseOf(breakdown.range, range) })}
        total={formatUsd(breakdown.totalCostUsd)}
        caption={t('ecosystem.breakdown.total')}
      />

      <p class="tabular mt-3 flex items-center gap-1.5 border-t border-line pt-3 text-xs text-fg-3">
        <Info size={16} class="shrink-0" aria-hidden="true" />
        {t('ecosystem.breakdown.footer', { cost: formatUsd(breakdown.totalCostUsd), tokens: tokensText(breakdown.totalTokens) })}
        {#if breakdown.rows.some((row) => row.estimatedPrice)}{t('ecosystem.breakdown.estimated')}{/if}
      </p>
    {/if}
  {/snippet}
</SlotCard>
