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
</script>

{#if slot.data !== null}
  {@const usage = slot.data}

  <div class={cx('grid gap-3 transition-opacity duration-[var(--dur-base)] @xl:grid-cols-2', slot.stale && 'opacity-60')} aria-busy={slot.stale}>
    {@render tile(0, 'Cost', Coins, formatUsd(usage.costUsd), costDetail)}
    {@render tile(1, 'Tokens', Layers, formatTokens(usage.tokens.total), tokenDetail)}
    {@render tile(2, 'Sessions', MessagesSquare, formatCount(usage.sessions), sessionDetail)}
    {@render tile(3, 'Cost per prompt', Gauge, usage.costPerPromptUsd === null ? '—' : formatUsd(usage.costPerPromptUsd), efficiencyDetail)}
  </div>

  {#snippet costDetail()}
    {#if usage.costChange !== null}
      {@const change = usage.costChange}
      {@const Arrow = change === 0 ? Minus : change > 0 ? ArrowUpRight : ArrowDownRight}

      <p class={cx('flex items-center gap-1 text-sm font-medium', change > 0 ? 'text-warning' : change < 0 ? 'text-success' : 'text-fg-3')}>
        <Arrow size={ICON.sm} aria-hidden="true" />
        <span class="tabular">{formatChange(change)}</span>
        <span class="font-normal text-fg-3">vs the period before</span>
      </p>
      {#if usage.previousCostUsd !== null}<p class="tabular text-xs text-fg-3">That was {formatUsd(usage.previousCostUsd)}</p>{/if}
    {:else}
      <p class="text-sm text-fg-3">No earlier period to compare with.</p>
    {/if}
  {/snippet}

  {#snippet tokenDetail()}
    <TokenBar tokens={usage.tokens} />
  {/snippet}

  {#snippet sessionDetail()}
    <p class="tabular text-sm text-fg-2">{formatCount(usage.prompts)} prompts</p>
    <p class="tabular text-xs text-fg-3">{formatCount(usage.modelCalls)} model calls</p>
  {/snippet}

  {#snippet efficiencyDetail()}
    <p class="tabular text-sm text-fg-2">{usage.cacheHitRate === null ? 'No cache data' : `${formatPercent(usage.cacheHitRate * 100)} served from cache`}</p>
    <p class="text-xs text-fg-3">Share of input tokens read from the prompt cache</p>
  {/snippet}
{:else if slot.status === 'error'}
  <div class="flex items-start gap-3 rounded-lg border border-line bg-danger-wash p-4 text-sm" role="alert">
    <CircleAlert class="mt-0.5 shrink-0 text-danger" size={16} aria-hidden="true" />
    <div class="flex flex-col items-start gap-2">
      <p class="text-fg-2">{slot.error}</p>
      <Button size="sm" onclick={onretry}>Try again</Button>
    </div>
  </div>
{:else}
  <div class="grid gap-3 @xl:grid-cols-2" role="status" aria-label="Loading usage">
    {#each [0, 1, 2, 3] as placeholder (placeholder)}
      <div class="flex flex-col gap-3 rounded-lg border border-line bg-surface-1 p-4">
        <Skeleton width="40%" height="0.875rem" />
        <Skeleton width="65%" height="2rem" />
        <Skeleton width="85%" height="0.875rem" />
      </div>
    {/each}
  </div>
{/if}

{#snippet tile(index: number, label: string, Icon: IconComponent, value: string, detail: Snippet)}
  <article
    aria-label="{label}, {phrase}"
    in:rise={{ y: 14, delay: index * 60, duration: 420 }}
    class="flex min-w-0 flex-col gap-2 rounded-lg border border-line bg-surface-1 p-4 shadow-raised"
  >
    <header class="flex items-center gap-2 text-sm text-fg-3">
      <span class="grid size-7 place-items-center rounded-sm bg-accent-wash text-accent-text"><Icon size={ICON.sm} aria-hidden="true" /></span>
      {label}
    </header>
    <p class="tabular text-3xl leading-none font-semibold tracking-tight text-fg">{value}</p>
    <div class="flex flex-col gap-0.5">{@render detail()}</div>
  </article>
{/snippet}
