<script lang="ts">
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import Badge from '$lib/components/Badge.svelte';
  import { formatPercent, formatUsd } from '$lib/dashboard/format';
  import type { Budget } from '$lib/dashboard/models';
  import { dashboard } from '$lib/stores/dashboard.svelte';

  import Meter from '../charts/Meter.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    index?: number;
  }

  let { index = 0 }: Props = $props();

  const STATUS: Record<string, { label: string; tone: 'success' | 'warning' | 'danger' }> = {
    ok: { label: 'On track', tone: 'success' },
    warning: { label: 'Close to the cap', tone: 'warning' },
    exceeded: { label: 'Over budget', tone: 'danger' },
    over: { label: 'Over budget', tone: 'danger' },
  };

  function name(budget: Budget): string {
    const scope = budget.scope.charAt(0).toUpperCase() + budget.scope.slice(1);
    const folder = budget.project?.split(/[\\/]/).filter(Boolean).at(-1);

    return folder ? `${scope} · ${folder}` : scope;
  }
</script>

<SlotCard title="Budgets" subtitle="Spending caps set in the dashboard" slot={dashboard.budgets} onretry={() => void dashboard.openPanel(true)} {index}>
  {#snippet children(budgets)}
    {#if budgets.length === 0}
      <p class="text-sm text-fg-3" role="status">No budgets are set. Add a cap in the dashboard to see it here.</p>
    {:else}
      <ul class="flex flex-col gap-4">
        {#each budgets as budget, row (row)}
          {@const status = STATUS[budget.status] ?? { label: budget.status, tone: 'neutral' as const }}
          {@const projectedOver = budget.projectedUsd !== null && budget.projectedUsd > budget.capUsd}

          <li class="flex flex-col gap-1.5">
            <div class="flex items-baseline justify-between gap-2 text-sm">
              <span class="min-w-0 truncate"><span class="font-medium text-fg">{name(budget)}</span> <span class="tabular text-fg-3">· {budget.period}</span></span>
              <Badge tone={status.tone}>{status.label}</Badge>
            </div>
            <Meter
              label="{name(budget)} budget"
              percent={budget.usedPercent}
              valueText={formatPercent(budget.usedPercent)}
              projectedPercent={budget.projectedUsd !== null && budget.capUsd > 0 ? (budget.projectedUsd / budget.capUsd) * 100 : null}
            />
            <p class="tabular text-xs text-fg-3">
              {formatUsd(budget.spentUsd)} of {formatUsd(budget.capUsd)}
              {#if budget.projectedUsd !== null}
                ·
                <span class={projectedOver ? 'font-medium text-warning' : ''}>
                  {#if projectedOver}<TriangleAlert size={16} class="-mt-0.5 inline" aria-hidden="true" />{/if}
                  projected {formatUsd(budget.projectedUsd)}{projectedOver ? ', over the cap' : ''}
                </span>
              {/if}
            </p>
          </li>
        {/each}
      </ul>
    {/if}
  {/snippet}
</SlotCard>
