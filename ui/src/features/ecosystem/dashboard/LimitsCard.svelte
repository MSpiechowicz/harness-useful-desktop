<script lang="ts">
  import Gauge from '@lucide/svelte/icons/gauge';
  import MoveRight from '@lucide/svelte/icons/move-right';
  import TrendingDown from '@lucide/svelte/icons/trending-down';
  import { onMount } from 'svelte';

  import { timeUntil } from '$lib/dashboard/format';
  import {
    formatAge,
    formatClock,
    groupLimits,
    leftShare,
    leftTone,
    paceOf,
    planSubject,
    providerHarness,
    recentGroups,
    runsOutBeforeReset,
    windowName,
  } from '$lib/dashboard/limits';
  import type { LimitWindow } from '$lib/dashboard/models';
  import { limitInk } from '$lib/dashboard/palette';
  import { travel } from '$lib/design/motion';
  import { dashboard } from '$lib/stores/dashboard.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import LimitBar from '../charts/LimitBar.svelte';
  import LimitPager from './LimitPager.svelte';
  import CardEmpty from './CardEmpty.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    index?: number;
  }

  let { index = 0 }: Props = $props();

  const idBase = uid('limits');

  const groups = $derived(recentGroups(groupLimits(dashboard.limits.data ?? [])));

  /** The shown provider is remembered by key, so a refresh stays on it and one that has gone falls back to the first. */
  let current = $state<string | null>(null);
  const page = $derived(Math.max(0, groups.findIndex((group) => group.key === current)));

  function show(next: number): void {
    current = groups[next]?.key ?? null;
  }

  /** The clock the countdowns and projections read; refreshed so an open panel does not go stale. */
  let now = $state(Date.now());

  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 30_000);

    return () => clearInterval(timer);
  });

  /** When the window resets, in words: it may already have, or never have said. */
  function resetText(window: LimitWindow): string {
    if (window.usedPercent === null) {
      return 'has reset since last seen';
    }

    const countdown = window.resetsAt ? timeUntil(window.resetsAt, now) : null;
    if (countdown) {
      return `resets ${countdown}`;
    }

    return window.resetsAt ? 'has reset since last seen' : 'no reset time given';
  }

  function paceText(outAt: number | null, out: boolean): string {
    return out && outAt !== null ? `On pace to run out around ${formatClock(outAt, now)}` : 'On pace to last until the reset';
  }
</script>

<SlotCard
  title="Plan limits"
  subtitle="What is left of each limit. The tick marks the time left."
  slot={dashboard.limits}
  onretry={() => void dashboard.openPanel(true)}
  {index}
>
  {#snippet actions()}
    {#if groups.length > 1}
      <LimitPager {groups} {page} onpage={show} {idBase} />
    {/if}
  {/snippet}

  {#snippet children(windows)}
    {#if windows.length === 0}
      <CardEmpty icon={Gauge} title="No plan limits recorded yet." hint="The dashboard reads them while your agents run." />
    {:else}
      <!-- Every provider sits in the same grid cell, so the card stays as tall as the tallest one and nothing jumps while paging. -->
      <div class="grid">
        {#each groups as group, position (group.key)}
          {@const shown = position === page}
          <section
            id="{idBase}-page-{position}"
            role={groups.length > 1 ? 'tabpanel' : undefined}
            aria-labelledby={groups.length > 1 ? `${idBase}-tab-${position}` : undefined}
            inert={!shown}
            aria-hidden={!shown}
            class={cx(
              'flex flex-col gap-3.5 [grid-area:1/1] transition-[opacity,transform,visibility] duration-[var(--dur-base)] ease-[var(--ease-out-expo)]',
              shown ? 'visible opacity-100' : 'invisible opacity-0',
            )}
            style:transform={shown ? 'none' : `translateY(${travel(6)}px)`}
          >
            <!-- The tabs already name the plan; the line names it only without tabs, and dates a stale reading. -->
            {#if groups.length === 1 || group.staleMinutes !== null}
              <header class="flex items-center gap-2.5">
                {#if groups.length === 1}
                  <h4 class="text-sm font-semibold text-fg">{planSubject(group.provider, group.plan)}</h4>
                {/if}
                {#if group.staleMinutes !== null}
                  <span class="ml-auto shrink-0 text-xs text-fg-3">as of {formatAge(group.staleMinutes)}</span>
                {/if}
              </header>
            {/if}

            <!-- Keyed by position: a provider may report several windows with the same name. -->
            {#each group.windows as window, row (row)}
              {@const left = leftShare(window.usedPercent)}
              {@const tone = leftTone(left)}
              {@const pace = paceOf(window, now)}
              {@const out = pace !== null && runsOutBeforeReset(pace, window.resetsAt)}
              {@const name = windowName(window.window, window.scope)}
              {@const percent = `${Math.round(left * 100)}% left`}
              {@const reset = resetText(window)}

              <div class="flex flex-col gap-1.5">
                <div class="flex items-baseline justify-between gap-3 text-xs">
                  <span class="min-w-0 truncate text-fg-2">{name}</span>
                  <span class="shrink-0 tabular text-fg-3">
                    <span class="font-semibold" style:color={limitInk(tone, theme.resolved)}>{percent}</span>
                    · {reset}
                  </span>
                </div>

                <LimitBar
                  label="{planSubject(group.provider, group.plan)} {name}: {percent}, {reset}{pace?.outAt != null ? `. ${paceText(pace.outAt, out)}` : ''}"
                  {left}
                  timeLeft={pace?.timeLeft ?? null}
                  provider={providerHarness(group.provider)}
                />

                {#if pace?.outAt != null}
                  <p class="flex items-center gap-1.5 text-xs" style:color={limitInk(out ? 'low' : 'ok', theme.resolved)}>
                    {#if out}
                      <TrendingDown class="size-3.5 shrink-0" aria-hidden="true" />
                    {:else}
                      <MoveRight class="size-3.5 shrink-0" aria-hidden="true" />
                    {/if}
                    {paceText(pace.outAt, out)}
                  </p>
                {/if}
              </div>
            {/each}
          </section>
        {/each}
      </div>
    {/if}
  {/snippet}
</SlotCard>
