<script lang="ts">
  import type { LimitMode, LimitView } from '$lib/dashboard/limit-dial';

  import type { GaugeKind } from '../charts/limit-gauge';
  import LimitGauge from '../charts/LimitGauge.svelte';
  import LimitChip from './LimitChip.svelte';
  import LimitMeter from './LimitMeter.svelte';

  interface Props {
    views: LimitView[];
    kind: GaugeKind;
    color: string;
    mode: LimitMode;
  }

  let { views, kind, color, mode }: Props = $props();

  // Four or more get smaller gauges and the short name of their window.
  const dense = $derived(views.length >= 4);
</script>

<!-- A limit: its gauge, name, status chip and reset line, in rows of fixed height so a missing chip moves nothing. A lone
     limit sits beside its details, several sit in a column each, up to four to a row in columns of one width so a short row
     does not spread out. -->
{#if views.length === 1}
  {@const view = views[0]!}
  <LimitMeter {view} class="my-auto flex items-center gap-[var(--limit-solo-gap)] px-1 py-0.5">
    <LimitGauge {kind} {color} dial={view.dial} {mode} small={false} />
    <div class="mb-1.5 flex min-w-0 flex-col items-start gap-1.5">
      <span class="text-sm font-medium text-fg">{view.name}</span>
      <span class="flex h-5 items-center">{#if view.chip}<LimitChip chip={view.chip} />{/if}</span>
      <span class="text-2xs text-fg-3" title={view.resetTitle}>{view.reset}</span>
    </div>
  </LimitMeter>
{:else}
  <div class="my-auto grid justify-items-center gap-x-2 gap-y-3" style:grid-template-columns="repeat({Math.min(views.length, 4)}, minmax(0, 1fr))">
    {#each views as view, row (row)}
      <LimitMeter {view} class="flex w-full min-w-0 flex-col items-center gap-1.5 text-center">
        <LimitGauge {kind} {color} dial={view.dial} {mode} small={dense} />
        <span class="mt-1.5 h-[var(--caption-line)] max-w-full truncate text-sm leading-[var(--caption-line)] font-medium text-fg" title={view.name}>{dense ? view.shortName : view.name}</span>
        <span class="flex h-5 max-w-full items-center justify-center">{#if view.chip}<LimitChip chip={view.chip} {dense} />{/if}</span>
        <span class="min-h-4 text-2xs text-balance text-fg-3" title={view.resetTitle}>{view.reset}</span>
      </LimitMeter>
    {/each}
  </div>
{/if}
