<script lang="ts">
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import { dashboard, type PanelRange } from '$lib/stores/dashboard.svelte';
  import { relativeTime } from '$lib/utils/format';

  import { causeWords } from './failures';
  import { phraseOf } from './ranges';
  import CardEmpty from './CardEmpty.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    range: PanelRange;
    index?: number;
  }

  let { range, index = 0 }: Props = $props();

  const LIMIT = 5;
  const phrase = $derived(phraseOf(dashboard.failures.data?.range, range));
</script>

<SlotCard title="Recent failures" subtitle="The latest failed tool calls in {phrase}" slot={dashboard.failures} onretry={() => void dashboard.openPanel(true)} {index}>
  {#snippet children(failures)}
    {#if failures.recent.length === 0}
      <CardEmpty icon={CircleCheck} tone="success" title="No tool call failed in this range." />
    {:else}
      <ol class="-mt-1 flex flex-col divide-y divide-line">
        {#each failures.recent.slice(0, LIMIT) as failure, row (row)}
          <li class="flex flex-col gap-1 py-2.5 last:pb-0">
            <p class="flex min-w-0 items-center gap-2 text-xs text-fg-3">
              <span class="min-w-0 truncate rounded-sm bg-surface-3 px-1.5 py-px font-mono text-fg-2" title={failure.tool}>{failure.tool}</span>
              <span class="shrink-0">{causeWords(failure.cause)}</span>
              <span class="ml-auto shrink-0">{relativeTime(failure.time)}</span>
            </p>
            <p class="line-clamp-2 font-mono text-xs break-words text-fg-2" title={failure.error}>{failure.error}</p>
          </li>
        {/each}
      </ol>
    {/if}
  {/snippet}
</SlotCard>
