<script lang="ts">
  import ArrowDownRight from '@lucide/svelte/icons/arrow-down-right';
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';
  import Coins from '@lucide/svelte/icons/coins';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Gauge from '@lucide/svelte/icons/gauge';
  import Layers from '@lucide/svelte/icons/layers';
  import MessagesSquare from '@lucide/svelte/icons/messages-square';
  import Minus from '@lucide/svelte/icons/minus';
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { formatChange, formatPercent, formatTokens, formatUsd } from '$lib/dashboard/format';
  import type { UsageSummary } from '$lib/dashboard/models';
  import { rise } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { PanelRange, QuerySlot } from '$lib/stores/dashboard.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  import TokenBar from '../charts/TokenBar.svelte';
  import { phraseOf } from './ranges';

  interface Props {
    slot: QuerySlot<UsageSummary>;
    range: PanelRange;
    onretry: () => void;
  }

  let { slot, range, onretry }: Props = $props();

  const phrase = $derived(phraseOf(slot.data?.range, range));

  // Every tile has the same four slots at fixed heights (label, value, detail), so no range, state
  // or number length can resize one. The grid, the skeleton and the error box all rely on these.
  const GRID = 'grid auto-rows-fr gap-3 @xl:grid-cols-2';
  const TILE = 'flex min-w-0 flex-col gap-2 rounded-lg border border-line bg-surface-1 p-4';
  const DETAIL = 'h-12';
</script>

{#if slot.data !== null}
  {@const usage = slot.data}

  <div class={cx(GRID, 'transition-opacity duration-[var(--dur-base)]', slot.stale && 'opacity-60')} aria-busy={slot.stale}>
    {@render tile(0, t('ecosystem.usage.cost'), Coins, formatUsd(usage.costUsd), costDetail)}
    {@render tile(1, t('ecosystem.usage.tokens'), Layers, formatTokens(usage.tokens.total), tokenDetail)}
    {@render tile(2, t('ecosystem.usage.sessions'), MessagesSquare, formatCount(usage.sessions), sessionDetail)}
    {@render tile(3, t('ecosystem.usage.perPrompt'), Gauge, usage.costPerPromptUsd === null ? '—' : formatUsd(usage.costPerPromptUsd), efficiencyDetail)}
  </div>

  {#snippet costDetail()}
    {#if usage.costChange !== null}
      {@const change = usage.costChange}
      {@const Arrow = change === 0 ? Minus : change > 0 ? ArrowUpRight : ArrowDownRight}

      <div class="flex h-6 min-w-0 items-center gap-2">
        <!-- More spend is the one to notice; less is good news. -->
        <span
          class={cx(
            'tabular inline-flex shrink-0 items-center gap-0.5 rounded-full py-0.5 pr-2 pl-1.5 text-xs font-medium',
            change > 0 ? 'bg-warning-wash text-warning' : change < 0 ? 'bg-success-wash text-success' : 'bg-surface-3 text-fg-3',
          )}
        >
          <Arrow size={14} aria-hidden="true" />
          {formatChange(change)}
        </span>
        <span class="truncate text-sm text-fg-3">{t('ecosystem.usage.vsPrevious')}</span>
      </div>
      <p class="tabular h-4 truncate text-xs leading-4 text-fg-3">
        {usage.previousCostUsd === null ? '' : t('ecosystem.usage.wasAmount', { amount: formatUsd(usage.previousCostUsd) })}
      </p>
    {:else}
      <p class="h-6 truncate text-sm leading-6 text-fg-3">{t('ecosystem.usage.noPrevious')}</p>
    {/if}
  {/snippet}

  {#snippet tokenDetail()}
    <TokenBar tokens={usage.tokens} />
  {/snippet}

  {#snippet sessionDetail()}
    <p class="tabular h-6 truncate text-sm leading-6 text-fg-2">{tn('ecosystem.count.prompt', usage.prompts)}</p>
    <p class="tabular h-4 truncate text-xs leading-4 text-fg-3">{tn('ecosystem.count.modelCall', usage.modelCalls)}</p>
  {/snippet}

  {#snippet efficiencyDetail()}
    {#if usage.cacheHitRate === null}
      <p class="h-6 truncate text-sm leading-6 text-fg-3">{t('ecosystem.usage.noCache')}</p>
    {:else}
      {@const share = Math.min(Math.max(usage.cacheHitRate, 0), 1)}

      <div class="flex h-6 min-w-0 items-center gap-2">
        <!-- The meter repeats the sentence beside it, so it stays out of the accessibility tree. -->
        <span class="h-1.5 w-14 shrink-0 overflow-hidden rounded-full bg-surface-3" aria-hidden="true">
          <span class="block h-full rounded-full bg-accent-text transition-[width] duration-[var(--dur-slow)]" style:width="{share * 100}%"></span>
        </span>
        <span class="tabular truncate text-sm text-fg-2">{t('ecosystem.usage.fromCache', { percent: formatPercent(share * 100) })}</span>
      </div>
    {/if}
    <p class="h-4 truncate text-xs leading-4 text-fg-3" title={t('ecosystem.usage.cacheShareTitle')}>{t('ecosystem.usage.cacheShare')}</p>
  {/snippet}
{:else if slot.status === 'error'}
  <!-- As tall as the two rows of tiles, so a failed load does not collapse the page under it. -->
  <div class="flex items-center gap-3 rounded-lg border border-line bg-danger-wash p-4 text-sm @xl:min-h-[20.75rem]" role="alert">
    <CircleAlert class="shrink-0 text-danger" size={ICON.md} aria-hidden="true" />
    <div class="flex flex-col items-start gap-2">
      <p class="text-fg-2">{slot.error}</p>
      <Button onclick={onretry}>{t('ecosystem.retry')}</Button>
    </div>
  </div>
{:else}
  <div class={GRID} role="status" aria-label={t('ecosystem.usage.loading')}>
    {#each [0, 1, 2, 3] as placeholder (placeholder)}
      <div class={cx(TILE, 'shadow-raised')}>
        <div class="flex h-7 items-center gap-2">
          <Skeleton width="1.75rem" height="1.75rem" class="rounded-sm" />
          <Skeleton width="30%" height="0.875rem" />
        </div>
        <div class="flex h-9 items-center"><Skeleton width="55%" height="1.75rem" /></div>
        <div class={cx(DETAIL, 'flex flex-col gap-2')}>
          <Skeleton width="80%" height="1rem" />
          <Skeleton width="50%" height="0.75rem" />
        </div>
      </div>
    {/each}
  </div>
{/if}

{#snippet tile(index: number, label: string, Icon: IconComponent, value: string, detail: Snippet)}
  <article aria-label={t('ecosystem.usage.tile', { label, phrase })} in:rise={{ y: 14, delay: index * 60, duration: 420 }} class={cx(TILE, 'shadow-raised')}>
    <header class="flex h-7 items-center gap-2 text-sm font-medium text-fg-2">
      <span class="grid size-7 shrink-0 place-items-center rounded-sm bg-accent-wash text-accent-text"><Icon size={ICON.sm} aria-hidden="true" /></span>
      <span class="truncate">{label}</span>
    </header>
    <p class="tabular h-9 truncate text-3xl leading-9 font-semibold tracking-tight text-fg">{value}</p>
    <div class={cx(DETAIL, 'flex flex-col gap-0.5')}>{@render detail()}</div>
  </article>
{/snippet}
