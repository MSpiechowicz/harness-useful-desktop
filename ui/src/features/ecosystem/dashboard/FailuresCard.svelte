<script lang="ts">
  import { formatShare } from '$lib/dashboard/format';
  import type { Failures } from '$lib/dashboard/models';
  import { dashboard, type PanelRange } from '$lib/stores/dashboard.svelte';
  import { formatCount, relativeTime } from '$lib/utils/format';

  import BarList from '../charts/BarList.svelte';
  import type { BarDatum } from '../charts/bars';
  import { ACCENT_COLOR } from '../charts/colors';
  import { phraseOf } from './ranges';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    range: PanelRange;
    index?: number;
  }

  let { range, index = 0 }: Props = $props();

  const phrase = $derived(phraseOf(dashboard.failures.data?.range, range));

  function words(cause: string): string {
    return cause.replaceAll('_', ' ');
  }

  function bars(failures: Failures): BarDatum[] {
    return failures.causes.map((cause) => ({
      key: cause.cause,
      label: words(cause.cause),
      value: cause.count,
      valueLabel: `${cause.count} · ${formatShare(cause.share)}`,
      description: `${words(cause.cause)}: ${cause.count} ${cause.count === 1 ? 'failure' : 'failures'}, ${formatShare(cause.share)}${cause.tools.length > 0 ? `, in ${cause.tools.join(', ')}` : ''}`,
      color: ACCENT_COLOR,
    }));
  }
</script>

<SlotCard title="Tool failures" subtitle="Why agent tool calls failed in {phrase}" slot={dashboard.failures} onretry={() => void dashboard.openPanel(true)} {index} class="@container">
  {#snippet children(failures)}
    {#if failures.failed === 0}
      <p class="text-sm text-fg-3" role="status">No tool call failed in this range.</p>
    {:else}
      <dl class="tabular grid grid-cols-3 gap-3 text-sm">
        <div class="flex flex-col"><dt class="text-fg-3">Failed</dt><dd class="text-xl font-semibold text-fg">{failures.failed} <span class="text-sm font-normal text-fg-3">of {formatCount(failures.calls)}</span></dd></div>
        <div class="flex flex-col"><dt class="text-fg-3">Error rate</dt><dd class="text-xl font-semibold text-fg">{formatShare(failures.errorRate)}</dd></div>
        <div class="flex flex-col"><dt class="text-fg-3">Declined</dt><dd class="text-xl font-semibold text-fg">{failures.declined}</dd></div>
      </dl>

      <div class="mt-4 flex flex-col gap-4 @2xl:grid @2xl:grid-cols-2 @2xl:gap-6">
        <div>
          <h4 class="mb-1 text-sm font-medium text-fg-3">Causes</h4>
          <BarList data={bars(failures)} label="Failures by cause" />
        </div>

        {#if failures.recent.length > 0}
          <div>
            <h4 class="mb-1 text-sm font-medium text-fg-3">Most recent</h4>
            <ol class="flex flex-col divide-y divide-line">
              {#each failures.recent.slice(0, 4) as failure, row (row)}
                <li class="flex flex-col gap-0.5 py-2 first:pt-1">
                  <p class="flex items-center gap-2 text-xs text-fg-3">
                    <span class="rounded-sm bg-surface-3 px-1.5 py-0.5 font-mono text-fg-2">{failure.tool}</span>
                    <span>{words(failure.cause)}</span>
                    <span class="ml-auto shrink-0">{relativeTime(failure.time)}</span>
                  </p>
                  <p class="line-clamp-2 font-mono text-xs break-words text-fg-2" title={failure.error}>{failure.error}</p>
                </li>
              {/each}
            </ol>
          </div>
        {/if}
      </div>
    {/if}
  {/snippet}
</SlotCard>
