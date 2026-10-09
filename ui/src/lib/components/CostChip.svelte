<script lang="ts">
  import Coins from '@lucide/svelte/icons/coins';
  import { onMount, untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { formatTokens, formatUsd } from '$lib/dashboard/format';
  import { rise } from '$lib/design/motion';
  import { dashboard, type CostTool } from '$lib/stores/dashboard.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  interface Props {
    /** What the cost is of: a chat session or a git branch. */
    kind: 'session' | 'branch';
    /** The dashboard arguments naming it; `null` when there is nothing to ask about yet. */
    args: Record<string, string> | null;
    /** Changes when new usage is likely (a finished turn); the chip then asks again. */
    refreshKey?: unknown;
    /** Which edge of the chip the details open from. */
    align?: 'start' | 'end';
    class?: string;
  }

  let { kind, args, refreshKey, align = 'end', class: className }: Props = $props();

  /** The dashboard rescans every 30 seconds, so asking more often only repeats an answer. */
  const POLL_MS = 60_000;

  const tool = $derived<CostTool>(kind === 'session' ? 'sessionCost' : 'branchCost');
  const entry = $derived(args ? dashboard.cost(tool, args) : undefined);
  const summary = $derived(entry?.status === 'unavailable' ? null : (entry?.summary ?? null));
  const noun = $derived(kind === 'session' ? 'Session' : 'Branch');

  let open = $state(false);
  let previousKey: unknown = untrack(() => refreshKey);

  // The first read honours the cache; a changed key (a turn ended) forces a fresh one.
  $effect(() => {
    if (!args) {
      return;
    }

    const changed = refreshKey !== previousKey;
    previousKey = refreshKey;
    void dashboard.requestCost(tool, args, changed);
  });

  onMount(() => {
    const timer = setInterval(() => {
      if (args) {
        void dashboard.requestCost(tool, args);
      }
    }, POLL_MS);

    return () => clearInterval(timer);
  });

  const rows = $derived.by(() => {
    if (!summary) {
      return [];
    }

    const lines: [string, string][] = [
      ['Tokens', formatTokens(summary.tokens)],
      ['Model calls', formatCount(summary.modelCalls)],
      ['Prompts', formatCount(summary.prompts)],
    ];

    if (summary.subagentCostUsd !== null) {
      lines.push(['Subagents', formatUsd(summary.subagentCostUsd)]);
    }
    if (summary.commits !== null) {
      lines.push(['Commits', String(summary.commits)]);
    }
    if (summary.linesAdded !== null) {
      lines.push(['Lines', `+${summary.linesAdded} −${summary.linesRemoved ?? 0}`]);
    }

    return lines;
  });
</script>

{#if summary}
  <span in:rise={{ y: 4, duration: 240 }} class={cx('inline-flex', className)}>
    <Popover label="{noun} cost" kind="dialog" {align} bind:open class="w-72">
      {#snippet trigger({ props })}
        <button
          type="button"
          {...props}
          aria-label="{noun} cost {formatUsd(summary.costUsd)}. Show details"
          class="tabular inline-flex h-8 items-center gap-1.5 rounded-md border border-line bg-surface-2 px-2.5 text-sm text-fg-2 transition-[background-color,color,transform] duration-[var(--dur-fast)] hover:bg-surface-3 hover:text-fg active:scale-[0.97]"
        >
          <Coins size={16} class="text-accent-text" aria-hidden="true" />
          {formatUsd(summary.costUsd)}
        </button>
      {/snippet}

      {#snippet children({ close })}
        <div class="flex flex-col gap-3 p-1.5">
          <div class="flex flex-col">
            <h3 class="text-sm font-semibold text-fg">{noun} cost</h3>
            {#if summary.title}<p class="truncate text-xs text-fg-3" title={summary.title}>{summary.title}</p>{/if}
          </div>

          <p class="tabular text-3xl leading-none font-semibold text-fg">{formatUsd(summary.costUsd)}</p>

          <dl class="tabular grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            {#each rows as [name, value] (name)}
              <dt class="text-fg-3">{name}</dt>
              <dd class="text-right text-fg">{value}</dd>
            {/each}
          </dl>

          {#if summary.models.length > 0}
            <ul class="tabular flex flex-col gap-0.5 border-t border-line pt-2 text-xs text-fg-2" aria-label="Cost by model">
              {#each summary.models.slice(0, 4) as model (model.model)}
                <li class="flex justify-between gap-3"><span class="truncate">{model.model}</span><span>{formatUsd(model.costUsd)}</span></li>
              {/each}
            </ul>
          {/if}

          <p class="text-xs text-fg-3">API list prices from your Harness Dashboard, not what a subscription bills.</p>

          <Button
            size="sm"
            onclick={() => {
              close();
              nav.go('ecosystem');
            }}>Open Ecosystem</Button
          >
        </div>
      {/snippet}
    </Popover>
  </span>
{/if}
