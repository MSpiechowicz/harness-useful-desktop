<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { formatPercent, timeUntil } from '$lib/dashboard/format';
  import type { LimitWindow } from '$lib/dashboard/models';
  import { dashboard } from '$lib/stores/dashboard.svelte';

  import Meter from '../charts/Meter.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    index?: number;
  }

  let { index = 0 }: Props = $props();

  const WINDOWS: Record<string, string> = { '5h': '5-hour window', '7d': '7-day window' };

  function title(window: LimitWindow): string {
    return `${window.provider} ${window.plan ?? ''}`.trim();
  }

  function tone(status: string): 'success' | 'warning' | 'neutral' {
    if (status === 'fresh') {
      return 'success';
    }

    return status === 'stale' ? 'warning' : 'neutral';
  }
</script>

<SlotCard title="Plan limits" subtitle="As last read by the dashboard" slot={dashboard.limits} onretry={() => void dashboard.openPanel(true)} {index}>
  {#snippet children(windows)}
    {#if windows.length === 0}
      <p class="text-sm text-fg-3" role="status">The dashboard has not recorded any plan limits yet.</p>
    {:else}
      <ul class="flex flex-col gap-4">
        {#each windows as window (`${window.provider}${window.window}`)}
          {@const reset = window.resetsAt ? timeUntil(window.resetsAt) : null}

          <li class="flex flex-col gap-1.5">
            <div class="flex items-baseline justify-between gap-2 text-sm">
              <span class="min-w-0 truncate"><span class="font-medium text-fg capitalize">{title(window)}</span> <span class="text-fg-3">· {WINDOWS[window.window] ?? window.window}</span></span>
              <Badge tone={tone(window.status)}>{window.status}</Badge>
            </div>
            <Meter
              label="{title(window)} {WINDOWS[window.window] ?? window.window}"
              percent={window.usedPercent}
              valueText={window.usedPercent === null ? '' : formatPercent(window.usedPercent)}
            />
            {#if window.usedPercent === null}
              <p class="text-xs text-fg-3">No current reading: the window reset after it was recorded.</p>
            {:else if reset}
              <p class="text-xs text-fg-3">Resets {reset}</p>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/snippet}
</SlotCard>
