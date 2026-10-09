<script lang="ts">

  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import RefreshButton from '$lib/components/RefreshButton.svelte';
  import { dashboard, PANEL_RANGES, type PanelRange } from '$lib/stores/dashboard.svelte';

  import BreakdownCard from './BreakdownCard.svelte';
  import BudgetsCard from './BudgetsCard.svelte';
  import FailuresCard from './FailuresCard.svelte';
  import LimitsCard from './LimitsCard.svelte';
  import TipsCard from './TipsCard.svelte';
  import UsageTiles from './UsageTiles.svelte';

  const LABELS: Record<PanelRange, string> = { today: 'Today', '7d': '7 days', '30d': '30 days' };
  const OPTIONS: StripOption<PanelRange>[] = PANEL_RANGES.map((range) => ({ value: range, label: LABELS[range] }));

  const reloading = $derived(dashboard.usage.status === 'loading');
</script>

<section aria-labelledby="usage-title" class="@container flex min-w-0 flex-col gap-4">
  <header class="flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
    <div class="flex min-w-0 flex-col">
      <h2 id="usage-title" class="text-xl text-fg">Your usage</h2>
      <p class="text-sm text-fg-3">From your local dashboard, at API list prices rather than a subscription bill.</p>
    </div>

    <div class="flex items-center gap-2">
      <SegmentedControl label="Time range" options={OPTIONS} bind:value={() => dashboard.range, (range) => void dashboard.setRange(range)} />
      <RefreshButton label="Refresh usage" busy={reloading} onclick={() => void dashboard.openPanel(true)} />
    </div>
  </header>

  <UsageTiles slot={dashboard.usage} range={dashboard.range} onretry={() => void dashboard.openPanel(true)} />

  <BreakdownCard range={dashboard.range} index={2} />

  <div class="grid gap-4 @3xl:grid-cols-2">
    <LimitsCard index={3} />
    <BudgetsCard index={4} />
  </div>

  <FailuresCard range={dashboard.range} index={4} />
  <TipsCard index={5} />
</section>
