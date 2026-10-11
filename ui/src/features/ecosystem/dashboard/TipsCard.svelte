<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Info from '@lucide/svelte/icons/info';
  import Lightbulb from '@lucide/svelte/icons/lightbulb';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import { formatUsd } from '$lib/dashboard/format';
  import { scaled } from '$lib/design/motion';
  import { dashboard } from '$lib/stores/dashboard.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import CardEmpty from './CardEmpty.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    index?: number;
  }

  let { index = 0 }: Props = $props();

  const COLLAPSED = 3;

  const listId = uid('tips');

  let expanded = $state(false);

  /** The tips' estimates added up, or null when none of them carries a figure. */
  function totalImpact(tips: { impactUsdPerMonth: number | null }[]): number | null {
    const priced = tips.filter((tip) => tip.impactUsdPerMonth !== null);

    if (priced.length === 0) {
      return null;
    }

    return priced.reduce((sum, tip) => sum + (tip.impactUsdPerMonth ?? 0), 0);
  }
</script>

<SlotCard title="Ways to spend less" subtitle="Suggestions from the last 30 days" slot={dashboard.tips} onretry={() => void dashboard.openPanel(true)} {index}>
  {#snippet actions()}
    {@const tips = dashboard.tips.data ?? []}
    {@const total = totalImpact(tips)}
    {#if total !== null}
      <!-- The tips can overlap, so the sum is an upper bound, not a promise. -->
      <p class="tabular flex shrink-0 flex-col items-end text-right">
        <span class="text-lg leading-6 font-semibold text-fg">≈ {formatUsd(total)}<span class="text-sm font-normal text-fg-3">/mo</span></span>
        <span class="text-xs text-fg-3">up to, across {tips.length} {tips.length === 1 ? 'suggestion' : 'suggestions'}</span>
      </p>
    {/if}
  {/snippet}

  {#snippet children(tips)}
    {#if tips.length === 0}
      <CardEmpty icon={Lightbulb} title="Nothing to suggest right now." />
    {:else}
      <div class="flex flex-col gap-4">
        <ul id={listId} class="mt-2 flex flex-col">
          {#each expanded ? tips : tips.slice(0, COLLAPSED) as tip, position (tip.id)}
            {@const warn = tip.severity === 'warn'}

            <li
              transition:slide={{ duration: position < COLLAPSED ? 0 : scaled(220) }}
              class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3.5 border-t border-line py-4 first:border-t-0 first:pt-0 last:pb-0"
            >
              <span class={cx('grid size-8 place-items-center rounded-md', warn ? 'bg-warning-wash text-warning' : 'bg-info-wash text-info')}>
                {#if warn}
                  <TriangleAlert size={16} aria-hidden="true" />
                {:else}
                  <Info size={16} aria-hidden="true" />
                {/if}
              </span>

              <div class="flex min-w-0 flex-col gap-1.5">
                <div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
                  <h4 class="min-w-48 flex-1 basis-64 text-sm leading-6 font-semibold text-fg">
                    <span class="sr-only">{warn ? 'Heads up: ' : 'Tip: '}</span>{tip.title}
                  </h4>

                  {#if tip.impactUsdPerMonth !== null}
                    <p class="tabular flex shrink-0 items-baseline gap-1 whitespace-nowrap">
                      <span class="sr-only">Estimated savings: </span>
                      <span class="text-base leading-6 font-semibold text-fg">≈ {formatUsd(tip.impactUsdPerMonth)}</span>
                      <span class="text-xs text-fg-3">/mo est.</span>
                    </p>
                  {/if}
                </div>

                <p class="max-w-prose text-sm leading-relaxed text-fg-2">{tip.detail}</p>
              </div>
            </li>
          {/each}
        </ul>

        {#if tips.length > COLLAPSED}
          <Button variant="ghost" size="sm" class="-mt-1 -ml-3 self-start" aria-expanded={expanded} aria-controls={listId} onclick={() => (expanded = !expanded)}>
            {expanded ? 'Show fewer' : `Show all ${tips.length}`}
            {#snippet trailing()}
              <ChevronDown size={16} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-base)]', expanded && 'rotate-180')} />
            {/snippet}
          </Button>
        {/if}
      </div>
    {/if}
  {/snippet}
</SlotCard>
