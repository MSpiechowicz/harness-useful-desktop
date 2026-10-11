<script lang="ts">
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import { formatShare } from '$lib/dashboard/format';
  import type { Failures } from '$lib/dashboard/models';
  import { dashboard, type PanelRange } from '$lib/stores/dashboard.svelte';
  import { formatCount } from '$lib/utils/format';

  import { causeColor } from '../charts/error-colors';
  import type { ShareSegment } from '../charts/share';
  import Waffle from '../charts/Waffle.svelte';
  import { causeShares, causeWords as words } from './failures';
  import { phraseOf } from './ranges';
  import CardEmpty from './CardEmpty.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    range: PanelRange;
    index?: number;
  }

  let { range, index = 0 }: Props = $props();

  const phrase = $derived(phraseOf(dashboard.failures.data?.range, range));

  function segments(failures: Failures): ShareSegment[] {
    return causeShares(failures.causes).map((part) => {
      const label = part.folded ? 'other' : words(part.key);
      const folded = part.folded ? part.causes.map(words).join(', ') : undefined;

      return {
        key: part.key,
        label,
        value: part.count,
        valueLabel: formatCount(part.count),
        shareLabel: formatShare(part.share),
        note: folded,
        description: `${folded ? `other (${folded})` : label}: ${part.count} ${part.count === 1 ? 'failure' : 'failures'}, ${formatShare(part.share)}${part.tools.length > 0 ? `, in ${part.tools.join(', ')}` : ''}`,
        color: causeColor(part.folded ? null : part.key),
      };
    });
  }
</script>

<SlotCard title="Tool failures" subtitle="Why agent tool calls failed in {phrase}" slot={dashboard.failures} onretry={() => void dashboard.openPanel(true)} {index} class="@container">
  {#snippet children(failures)}
    {#if failures.failed === 0}
      <CardEmpty icon={CircleCheck} tone="success" title="No tool call failed in this range." />
    {:else}
      <dl class="tabular flex flex-wrap gap-x-10 gap-y-3 text-sm">
        <div class="flex flex-col"><dt class="text-fg-3">Failed</dt><dd class="text-xl font-semibold text-fg">{failures.failed} <span class="text-sm font-normal text-fg-3">of {formatCount(failures.calls)}</span></dd></div>
        <div class="flex flex-col"><dt class="text-fg-3">Error rate</dt><dd class="text-xl font-semibold text-fg">{formatShare(failures.errorRate)}</dd></div>
        <div class="flex flex-col"><dt class="text-fg-3">Declined</dt><dd class="text-xl font-semibold text-fg">{failures.declined}</dd></div>
      </dl>

      <div class="mt-2">
        <h4 class="mb-3 text-sm font-medium text-fg-3">Causes</h4>
        <Waffle segments={segments(failures)} label="Failures by cause" />
      </div>
    {/if}
  {/snippet}
</SlotCard>
