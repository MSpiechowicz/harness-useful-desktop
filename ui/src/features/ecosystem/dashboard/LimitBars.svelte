<script lang="ts">
  import type { LimitMode, LimitView } from '$lib/dashboard/limit-dial';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import LimitBar from '../charts/LimitBar.svelte';
  import LimitChip from './LimitChip.svelte';
  import LimitMeter from './LimitMeter.svelte';

  interface Props {
    views: LimitView[];
    color: string;
    mode: LimitMode;
  }

  let { views, color, mode }: Props = $props();

  // Up to three get a header with the number, the bar and a line for the chip and the reset. Four or more get a one-line
  // header and a thinner bar.
  const dense = $derived(views.length >= 4);
</script>

{#snippet reset(view: LimitView)}
  <span title={view.resetTitle}>{view.reset}</span>
{/snippet}

<!-- A provider's limits as bars, any number: in columns of at least one width, as many as fit the card. Three or more spread
     over the card's height, fewer stay at the top like the first rows of three. -->
<div
  class={cx('grid flex-1 gap-x-8 gap-y-2 px-1', views.length >= 3 ? 'content-evenly' : 'content-start')}
  style:grid-template-columns="repeat(auto-fit, minmax(min({dense ? 'var(--limit-bars-col-dense)' : 'var(--limit-bars-col)'}, 100%), 1fr))"
>
  {#each views as view, row (row)}
    <LimitMeter {view} class="min-w-0">
      {#if dense}
        <div class="mb-1 flex h-5 min-w-0 items-center gap-2">
          <span class="max-w-[35%] shrink-0 truncate text-sm leading-5 font-medium text-fg" title={view.name}>{view.shortName}</span>
          <span class="min-w-0 truncate text-2xs text-fg-3">{@render reset(view)}</span>
          {#if view.chip}<span class="ml-auto shrink-0"><LimitChip chip={view.chip} dense /></span>{/if}
          <span class="min-w-[4ch] shrink-0 text-right text-base leading-5 font-bold tracking-[-0.02em] text-fg tabular {view.chip ? '' : 'ml-auto'}">{view.percent}%</span>
        </div>
        <LimitBar {color} dial={view.dial} {mode} small />
      {:else}
        <div class="mb-1 flex min-w-0 items-baseline gap-2">
          <span class="min-w-0 truncate text-sm leading-5 font-medium text-fg" title={view.name}>{view.name}</span>
          <span class="ml-auto shrink-0 text-lg leading-5 font-bold tracking-[-0.02em] text-fg tabular">{view.percent}%</span>
          <span class="shrink-0 text-2xs font-medium text-fg-2">{t(mode === 'used' ? 'ecosystem.limits.dial.used' : 'ecosystem.limits.dial.left')}</span>
        </div>
        <LimitBar {color} dial={view.dial} {mode} small={false} />
        <div class="mt-2 flex h-5 min-w-0 items-center gap-2.5">
          {#if view.chip}<span class="shrink-0"><LimitChip chip={view.chip} /></span>{/if}
          <span class="min-w-0 truncate text-2xs text-fg-3">{@render reset(view)}</span>
        </div>
      {/if}
    </LimitMeter>
  {/each}
</div>
