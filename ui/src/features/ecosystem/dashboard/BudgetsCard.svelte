<script lang="ts">
  import Wallet from '@lucide/svelte/icons/wallet';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import Badge from '$lib/components/Badge.svelte';
  import { formatPercent, formatUsd } from '$lib/dashboard/format';
  import type { Budget } from '$lib/dashboard/models';
  import { t } from '$lib/i18n/i18n.svelte';
  import { dashboard } from '$lib/stores/dashboard.svelte';

  import Meter from '../charts/Meter.svelte';
  import CardEmpty from './CardEmpty.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    index?: number;
  }

  let { index = 0 }: Props = $props();

  const STATUS: Record<string, { label: () => string; tone: 'success' | 'warning' | 'danger' }> = {
    ok: { label: () => t('ecosystem.budgets.ok'), tone: 'success' },
    warning: { label: () => t('ecosystem.budgets.warning'), tone: 'warning' },
    exceeded: { label: () => t('ecosystem.budgets.over'), tone: 'danger' },
    over: { label: () => t('ecosystem.budgets.over'), tone: 'danger' },
  };

  function name(budget: Budget): string {
    const scope = budget.scope.charAt(0).toUpperCase() + budget.scope.slice(1);
    const folder = budget.project?.split(/[\\/]/).filter(Boolean).at(-1);

    return folder ? `${scope} · ${folder}` : scope;
  }
</script>

<SlotCard title={t('ecosystem.budgets.title')} subtitle={t('ecosystem.budgets.subtitle')} slot={dashboard.budgets} onretry={() => void dashboard.openPanel(true)} {index}>
  {#snippet children(budgets)}
    {#if budgets.length === 0}
      <CardEmpty icon={Wallet} title={t('ecosystem.budgets.empty')} hint={t('ecosystem.budgets.emptyHint')} />
    {:else}
      <ul class="flex flex-col gap-4">
        {#each budgets as budget, row (row)}
          {@const status = STATUS[budget.status] ?? { label: () => budget.status, tone: 'neutral' as const }}
          {@const projectedOver = budget.projectedUsd !== null && budget.projectedUsd > budget.capUsd}

          <li class="flex flex-col gap-1.5">
            <div class="flex items-baseline justify-between gap-2 text-sm">
              <span class="min-w-0 truncate"><span class="font-medium text-fg">{name(budget)}</span> <span class="tabular text-fg-3">· {budget.period}</span></span>
              <Badge tone={status.tone}>{status.label()}</Badge>
            </div>
            <Meter
              label={t('ecosystem.budgets.meter', { name: name(budget) })}
              percent={budget.usedPercent}
              valueText={formatPercent(budget.usedPercent)}
              projectedPercent={budget.projectedUsd !== null && budget.capUsd > 0 ? (budget.projectedUsd / budget.capUsd) * 100 : null}
            />
            <p class="tabular text-xs text-fg-3">
              {t('ecosystem.budgets.of', { spent: formatUsd(budget.spentUsd), cap: formatUsd(budget.capUsd) })}
              {#if budget.projectedUsd !== null}
                ·
                <span class={projectedOver ? 'font-medium text-warning' : ''}>
                  {#if projectedOver}<TriangleAlert size={16} class="-mt-0.5 inline" aria-hidden="true" />{/if}
                  {t(projectedOver ? 'ecosystem.budgets.projectedOver' : 'ecosystem.budgets.projected', { amount: formatUsd(budget.projectedUsd) })}
                </span>
              {/if}
            </p>
          </li>
        {/each}
      </ul>
    {/if}
  {/snippet}
</SlotCard>
