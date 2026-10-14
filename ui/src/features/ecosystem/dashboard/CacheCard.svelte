<script lang="ts">
  import Database from '@lucide/svelte/icons/database';

  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { formatPercent, formatShare, formatTokens } from '$lib/dashboard/format';
  import type { UsageSummary } from '$lib/dashboard/models';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { dashboard, type PanelDimension, type PanelRange } from '$lib/stores/dashboard.svelte';
  import { cx } from '$lib/utils/cx';

  import Donut from '../charts/Donut.svelte';
  import type { ShareSegment } from '../charts/share';
  import { cacheMix, cacheRanking, type CacheMix } from './cache';
  import CardEmpty from './CardEmpty.svelte';
  import { phraseOf } from './ranges';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    range: PanelRange;
    index?: number;
  }

  let { range, index = 0 }: Props = $props();

  /** The most models listed under the ring; the breakdown card above them already holds the long tail. */
  const SHOWN = 5;

  // The same switch as the spend card, but its own: each card groups on its own.
  let dimension = $state<PanelDimension>('model');

  const DIMENSIONS = $derived<StripOption<PanelDimension>[]>([
    { value: 'model', label: t('ecosystem.breakdown.byModel') },
    { value: 'provider', label: t('ecosystem.breakdown.byHarness') },
  ]);

  const by = $derived(dimension === 'model' ? 'model' : 'provider');
  const grouped = $derived(dimension === 'model' ? dashboard.byModel : dashboard.byProvider);

  const phrase = $derived(phraseOf(dashboard.usage.data?.range, range));
  // The list loads on its own query: show it only while it covers the same range as the ring, dimmed when it reloads.
  const ranking = $derived(
    grouped.data !== null && grouped.data.range === dashboard.usage.data?.range ? cacheRanking(grouped.data.rows, SHOWN) : [],
  );

  function segments(mix: CacheMix): ShareSegment[] {
    const parts = [
      { key: 'read', label: t('ecosystem.tokens.cacheRead'), value: mix.read, color: 'var(--accent-2)' },
      { key: 'write', label: t('ecosystem.tokens.cacheWrite'), value: mix.write, color: 'var(--accent-3)' },
      { key: 'fresh', label: t('ecosystem.cache.fresh'), value: mix.fresh, color: 'var(--text-3)' },
    ];

    return parts.map((part) => {
      const share = formatShare(part.value / mix.total);
      const tokens = tn('ecosystem.count.token', part.value, { tokens: formatTokens(part.value) });

      return { ...part, valueLabel: formatTokens(part.value), shareLabel: share, description: `${part.label}: ${tokens}, ${share}` };
    });
  }

  function load(): void {
    void dashboard.openPanel(true);
  }
</script>

<SlotCard title={t('ecosystem.cache.title')} subtitle={t(`ecosystem.cache.subtitle.${by}`, { phrase })} slot={dashboard.usage} onretry={load} {index} class="@container">
  {#snippet actions()}
    <SegmentedControl label={t('ecosystem.cache.group')} options={DIMENSIONS} bind:value={dimension} />
  {/snippet}

  {#snippet children(usage: UsageSummary)}
    {@const mix = cacheMix(usage.tokens, usage.cacheHitRate)}

    {#if mix.total === 0 || mix.hitRate === null}
      <CardEmpty icon={Database} title={t('ecosystem.cache.empty')} />
    {:else}
      <Donut
        segments={segments(mix)}
        label={t('ecosystem.cache.label', { phrase: phraseOf(usage.range, range) })}
        total={formatPercent(mix.hitRate * 100)}
        caption={t('ecosystem.cache.caption')}
      />

      {#if ranking.length > 0}
        <div class={cx('mt-3 border-t border-line pt-3 transition-opacity duration-[var(--dur-base)]', grouped.stale && 'opacity-60')}>
          <h4 class="mb-1.5 text-xs font-medium text-fg-3">{t(`ecosystem.cache.by${by === 'model' ? 'Model' : 'Provider'}`)}</h4>
          <ul class="tabular flex flex-col gap-1.5 text-sm" aria-label={t(`ecosystem.cache.by${by === 'model' ? 'Model' : 'Provider'}`)}>
            {#each ranking as row (row.key)}
              <li class="grid grid-cols-[minmax(0,1fr)_5rem_3rem] items-center gap-x-3" aria-label={t('ecosystem.cache.row', { name: row.key, percent: formatPercent(row.hitRate * 100) })}>
                <span class="truncate text-fg-2" title={row.key}>{row.key}</span>
                <span class="h-1.5 overflow-hidden rounded-full bg-surface-3" aria-hidden="true">
                  <span class="block h-full rounded-full bg-accent-text transition-[width] duration-[var(--dur-slow)]" style:width="{row.hitRate * 100}%"></span>
                </span>
                <span class="text-right text-fg">{formatPercent(row.hitRate * 100)}</span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      <p class="tabular mt-3 border-t border-line pt-3 text-xs text-fg-3">
        {t('ecosystem.cache.footer', { tokens: tn('ecosystem.count.token', mix.total, { tokens: formatTokens(mix.total) }) })}
      </p>
    {/if}
  {/snippet}
</SlotCard>
