<script lang="ts">

  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t } from '$lib/i18n/i18n.svelte';
  import { dashboard, PANEL_RANGES, type PanelRange } from '$lib/stores/dashboard.svelte';

  import BreakdownCard from './BreakdownCard.svelte';
  import BudgetsCard from './BudgetsCard.svelte';
  import CacheCard from './CacheCard.svelte';
  import FailuresCard from './FailuresCard.svelte';
  import LimitsCard from './LimitsCard.svelte';
  import RecentFailuresCard from './RecentFailuresCard.svelte';
  import TipsCard from './TipsCard.svelte';
  import UsageTiles from './UsageTiles.svelte';

  const LABELS: Record<PanelRange, () => string> = {
    today: () => t('ecosystem.range.today'),
    '7d': () => t('ecosystem.range.7d'),
    '30d': () => t('ecosystem.range.30d'),
  };
  const OPTIONS = $derived<StripOption<PanelRange>[]>(PANEL_RANGES.map((range) => ({ value: range, label: LABELS[range]() })));
</script>

<section aria-labelledby="usage-title" class="@container flex min-w-0 flex-col gap-4">
  <header class="flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
    <div class="flex min-w-0 flex-col">
      <h2 id="usage-title" class="text-xl text-fg">{t('ecosystem.panel.title')}</h2>
      <p class="text-sm text-fg-3">{t('ecosystem.panel.subtitle')}</p>
    </div>

    <!-- Refreshing lives in the view header, which reloads the companions and the dashboard together. -->
    <SegmentedControl label={t('ecosystem.panel.range')} options={OPTIONS} bind:value={() => dashboard.range, (range) => void dashboard.setRange(range)} />
  </header>

  <UsageTiles slot={dashboard.usage} range={dashboard.range} onretry={() => void dashboard.openPanel(true)} />

  <div class="grid gap-4 @3xl:grid-cols-2">
    <BreakdownCard range={dashboard.range} index={2} />
    <CacheCard range={dashboard.range} index={3} />
  </div>

  <div class="grid gap-4 @3xl:grid-cols-2">
    <LimitsCard index={4} />
    <BudgetsCard index={5} />
  </div>

  <div class="grid gap-4 @3xl:grid-cols-2">
    <FailuresCard range={dashboard.range} index={6} />
    <RecentFailuresCard range={dashboard.range} index={7} />
  </div>
  <TipsCard index={8} />
</section>
