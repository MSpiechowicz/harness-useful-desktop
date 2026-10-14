<script lang="ts">
  import Coins from '@lucide/svelte/icons/coins';
  import { onMount, untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { formatTokens, formatUsd } from '$lib/dashboard/format';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
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
    /** Which side of the chip the details open on; `top` near the bottom of the window. */
    side?: 'top' | 'bottom';
    /** Plain text beside other small meters instead of a bordered chip. */
    quiet?: boolean;
    class?: string;
  }

  let { kind, args, refreshKey, align = 'end', side = 'bottom', quiet = false, class: className }: Props = $props();

  /** The dashboard rescans every 30 seconds, so asking more often only repeats an answer. */
  const POLL_MS = 60_000;

  const tool = $derived<CostTool>(kind === 'session' ? 'sessionCost' : 'branchCost');
  const entry = $derived(args ? dashboard.cost(tool, args) : undefined);
  const summary = $derived(entry?.status === 'unavailable' ? null : (entry?.summary ?? null));

  const title = $derived(t(kind === 'session' ? 'components.cost.title.session' : 'components.cost.title.branch'));

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
      [t('components.cost.tokens'), formatTokens(summary.tokens)],
      [t('components.cost.calls'), formatCount(summary.modelCalls)],
      [t('components.cost.prompts'), formatCount(summary.prompts)],
    ];

    if (summary.subagentCostUsd !== null) {
      lines.push([t('components.cost.subagents'), formatUsd(summary.subagentCostUsd)]);
    }
    if (summary.commits !== null) {
      lines.push([t('components.cost.commits'), String(summary.commits)]);
    }
    if (summary.linesAdded !== null) {
      lines.push([t('components.cost.lines'), `+${summary.linesAdded} −${summary.linesRemoved ?? 0}`]);
    }

    return lines;
  });
</script>

{#if summary}
  <span in:rise={{ y: 4, duration: 240 }} class={cx('inline-flex', className)}>
    <Popover label={title} kind="dialog" {align} {side} bind:open class="w-72">
      {#snippet trigger({ props })}
        <button
          type="button"
          {...props}
          aria-label={t(kind === 'session' ? 'components.cost.aria.session' : 'components.cost.aria.branch', { amount: formatUsd(summary.costUsd) })}
          class={cx(
            'tabular inline-flex h-control items-center gap-1.5 rounded-md transition-[background-color,color,transform] duration-[var(--dur-fast)] active:scale-[0.97]',
            quiet ? 'px-1.5 text-xs text-fg-3 hover:bg-hover hover:text-fg' : 'border border-line bg-surface-2 px-2.5 text-sm text-fg-2 hover:bg-surface-3 hover:text-fg',
          )}
        >
          <Coins size={16} class={quiet ? undefined : 'text-accent-text'} aria-hidden="true" />
          {formatUsd(summary.costUsd)}
        </button>
      {/snippet}

      {#snippet children({ close })}
        <div class="flex flex-col gap-3 p-1.5">
          <div class="flex flex-col">
            <h3 class="text-sm font-semibold text-fg">{title}</h3>
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
            <ul class="tabular flex flex-col gap-0.5 border-t border-line pt-2 text-xs text-fg-2" aria-label={t('components.cost.byModel')}>
              {#each summary.models.slice(0, 4) as model (model.model)}
                <li class="flex justify-between gap-3"><span class="truncate">{model.model}</span><span>{formatUsd(model.costUsd)}</span></li>
              {/each}
            </ul>
          {/if}

          <p class="text-xs text-fg-3">{t('components.cost.note')}</p>

          <Button
            onclick={() => {
              close();
              nav.go('ecosystem');
            }}>{t('components.cost.open')}</Button
          >
        </div>
      {/snippet}
    </Popover>
  </span>
{/if}
