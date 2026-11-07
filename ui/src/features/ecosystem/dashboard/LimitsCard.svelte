<script lang="ts">
  import Gauge from '@lucide/svelte/icons/gauge';
  import { onMount } from 'svelte';

  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { gaugeOptions, limitView, type LimitView } from '$lib/dashboard/limit-dial';
  import { formatAge, groupLimits, planSubject, providerHarness, recentGroups, type LimitGroup } from '$lib/dashboard/limits';
  import { travel } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { dashboard } from '$lib/stores/dashboard.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { cx } from '$lib/utils/cx';

  import { providerColor } from '../charts/colors';
  import CardEmpty from './CardEmpty.svelte';
  import LimitBars from './LimitBars.svelte';
  import LimitDial from './LimitDial.svelte';
  import LimitLegend from './LimitLegend.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    index?: number;
  }

  let { index = 0 }: Props = $props();

  // What the charts show: the share used or left, whether they draw where each limit is heading by the reset, and the style.
  const mode = $derived(prefs.value?.limitBars ?? 'used');
  const forecast = $derived(prefs.value?.limitForecast ?? true);
  const gauge = $derived(prefs.value?.limitGauge ?? 'bars');

  const groups = $derived(recentGroups(groupLimits(dashboard.limits.data ?? [])));

  /** The shown provider is remembered by key, so a refresh stays on it and one that has gone falls back to the first. */
  let current = $state<string | null>(null);
  const page = $derived(Math.max(0, groups.findIndex((group) => group.key === current)));
  const providerOptions = $derived(groups.map((group) => ({ value: group.key, label: planSubject(group.provider, group.plan) })));

  /** The clock the countdowns and projections read; refreshed so an open panel does not go stale. */
  let now = $state(Date.now());

  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 30_000);

    return () => clearInterval(timer);
  });

  interface Page {
    group: LimitGroup;
    color: string;
    views: LimitView[];
    /** The key to the charts: only where some limit has a forecast to explain. */
    legend: boolean;
  }

  const pages = $derived<Page[]>(
    groups.map((group) => {
      const subject = planSubject(group.provider, group.plan);
      const views = group.windows.map((window) => limitView(window, subject, mode, forecast, now));

      return { group, color: providerColor(providerHarness(group.provider)), views, legend: forecast && views.some((view) => view.pace !== null) };
    }),
  );

  const shownGroup = $derived(groups[page]);
</script>

<SlotCard
  title={t('ecosystem.limits.title')}
  subtitle={t(mode === 'used' ? 'ecosystem.limits.hintUsed' : 'ecosystem.limits.hintLeft')}
  slot={dashboard.limits}
  onretry={() => void dashboard.openPanel(true)}
  {index}
>
  {#snippet children(windows)}
    {#if windows.length === 0}
      <CardEmpty icon={Gauge} title={t('ecosystem.limits.empty')} hint={t('ecosystem.limits.emptyHint')} />
    {:else}
      <div class="flex flex-col gap-5">
        <!-- Half the card each. The word above a dropdown is for the eye: its button already says it to a screen reader. A stale
             reading is dated on the provider's line, so it takes no room from the charts. -->
        <div class="grid grid-cols-2 gap-3">
          <div class="flex min-w-0 flex-col gap-1">
            <div class="flex items-baseline justify-between gap-2 text-xs text-fg-3">
              <span class="shrink-0" aria-hidden="true">{t('ecosystem.limits.provider')}</span>
              {#if shownGroup && shownGroup.staleMinutes !== null}
                <span class="min-w-0 truncate">{t('ecosystem.limits.asOf', { age: formatAge(shownGroup.staleMinutes) })}</span>
              {/if}
            </div>
            {#if groups.length > 1}
              <FilterMenu label={t('ecosystem.limits.provider')} icon={null} steady fill align="start" options={providerOptions} value={shownGroup?.key ?? ''} onchange={(key) => (current = key)} />
            {:else if shownGroup}
              <h4 class="flex h-control items-center truncate text-base font-medium text-fg">{planSubject(shownGroup.provider, shownGroup.plan)}</h4>
            {/if}
          </div>

          <div class="flex min-w-0 flex-col gap-1">
            <span class="text-xs text-fg-3" aria-hidden="true">{t('ecosystem.limits.gauge')}</span>
            <FilterMenu label={t('ecosystem.limits.gauge')} icon={null} steady fill align="start" options={gaugeOptions()} value={gauge} onchange={(limitGauge) => void prefs.update({ limitGauge })} />
          </div>
        </div>

        <!-- One height whatever the provider or the chart style, so choosing either moves nothing, and no taller than the tallest
             chart. Every provider sits in the same grid cell, and more limits than fit scroll inside it. The key to the charts adds
             its own height when any provider has one. -->
        <div class="grid grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)]" style:height={pages.some((entry) => entry.legend) ? 'calc(var(--limit-body-h) + var(--limit-head-h) + var(--limit-head-gap))' : 'var(--limit-body-h)'}>
          {#each pages as entry, position (entry.group.key)}
            {@const shown = position === page}
            <section
              inert={!shown}
              aria-hidden={!shown}
              class={cx(
                'flex min-h-0 min-w-0 flex-col gap-[var(--limit-head-gap)] [grid-area:1/1] transition-[opacity,transform,visibility] duration-[var(--dur-base)] ease-[var(--ease-out-expo)]',
                shown ? 'visible opacity-100' : 'invisible opacity-0',
              )}
              style:transform={shown ? 'none' : `translateY(${travel(6)}px)`}
            >
              {#if entry.legend}
                <header class="flex min-h-[var(--limit-head-h)] items-center justify-end">
                  <LimitLegend color={entry.color} {mode} battery={gauge === 'battery'} />
                </header>
              {/if}

              <div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
                {#if gauge === 'bars'}
                  <LimitBars views={entry.views} color={entry.color} {mode} />
                {:else}
                  <LimitDial views={entry.views} kind={gauge} color={entry.color} {mode} />
                {/if}
              </div>
            </section>
          {/each}
        </div>
      </div>
    {/if}
  {/snippet}
</SlotCard>
