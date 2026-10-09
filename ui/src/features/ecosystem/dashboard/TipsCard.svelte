<script lang="ts">
  import Info from '@lucide/svelte/icons/info';
  import Lightbulb from '@lucide/svelte/icons/lightbulb';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { formatUsd } from '$lib/dashboard/format';
  import { dashboard } from '$lib/stores/dashboard.svelte';

  import SlotCard from './SlotCard.svelte';

  interface Props {
    index?: number;
  }

  let { index = 0 }: Props = $props();

  const COLLAPSED = 3;

  let expanded = $state(false);
</script>

<SlotCard title="Ways to spend less" subtitle="Suggestions from the last 30 days" slot={dashboard.tips} onretry={() => void dashboard.openPanel(true)} {index}>
  {#snippet children(tips)}
    {#if tips.length === 0}
      <p class="flex items-center gap-2 text-sm text-fg-3" role="status"><Lightbulb size={16} aria-hidden="true" />Nothing to suggest right now.</p>
    {:else}
      <ul class="flex flex-col divide-y divide-line">
        {#each expanded ? tips : tips.slice(0, COLLAPSED) as tip (tip.id)}
          {@const warn = tip.severity === 'warn'}

          <li class="flex flex-col gap-1 py-3 first:pt-0 last:pb-0">
            <div class="flex items-start gap-2">
              {#if warn}
                <TriangleAlert size={16} class="mt-0.5 shrink-0 text-warning" aria-hidden="true" />
              {:else}
                <Info size={16} class="mt-0.5 shrink-0 text-info" aria-hidden="true" />
              {/if}
              <p class="min-w-0 flex-1 text-sm font-medium text-fg"><span class="sr-only">{warn ? 'Heads up: ' : 'Tip: '}</span>{tip.title}</p>
              {#if tip.impactUsdPerMonth !== null}
                <Badge tone={warn ? 'warning' : 'accent'} class="tabular shrink-0">≈ {formatUsd(tip.impactUsdPerMonth)}/mo</Badge>
              {/if}
            </div>
            <p class="pl-6 text-sm text-fg-2">{tip.detail}</p>
          </li>
        {/each}
      </ul>

      {#if tips.length > COLLAPSED}
        <Button variant="ghost" size="sm" class="mt-2 -ml-2 self-start" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
          {expanded ? 'Show fewer' : `Show all ${tips.length}`}
        </Button>
      {/if}
    {/if}
  {/snippet}
</SlotCard>
