<script lang="ts">
  import { toolStepCount } from '$lib/chat/subagent-steps';
  import type { SubagentStepEntry } from '$lib/chat/types';
  import { formatTokens, formatUsd } from '$lib/dashboard/format';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';

  interface Props {
    subagent: Subagent;
    entries: SubagentStepEntry[];
  }

  let { subagent, entries }: Props = $props();

  interface Stat {
    label: string;
    value: string;
    /** 0 to 1: the figure is a share of something and draws a bar. */
    fraction?: number;
  }

  const context = $derived(subagent.contextTokens !== null && subagent.contextWindow ? Math.min(1, subagent.contextTokens / subagent.contextWindow) : null);
  // A figure the harness did not report is left out, not shown as a placeholder.
  const stats = $derived.by((): Stat[] => {
    const shown: Stat[] = [];

    if (subagent.tokens !== null) {
      shown.push({ label: t('subagents.stat.tokens'), value: formatTokens(subagent.tokens) });
    }
    if (subagent.costUsd !== null) {
      shown.push({ label: t('subagents.stat.cost'), value: formatUsd(subagent.costUsd) });
    }
    if (context !== null) {
      shown.push({ label: t('subagents.stat.context'), value: `${Math.round(context * 100)}%`, fraction: context });
    }

    return shown;
  });
  const toolCalls = $derived(subagent.toolCount ?? toolStepCount(entries));
</script>

<!-- The figures of the run and the heading of its steps: they stay in place above the steps, which scroll under them. -->
{#if stats.length > 0}
  <dl class="grid grid-cols-3 gap-2 pb-[var(--panel-block-gap)]">
    {#each stats as stat (stat.label)}
      <div class="flex min-w-0 flex-col-reverse justify-end rounded-[10px] border border-line bg-surface-2 px-2.5 py-2">
        {#if stat.fraction !== undefined}
          <div class="mt-1.5 h-1 overflow-hidden rounded-full bg-surface-3" role="presentation">
            <div class="h-full origin-left bg-accent transition-transform duration-[var(--dur-slow)]" style:transform="scaleX({stat.fraction})"></div>
          </div>
        {/if}
        <dd class="tabular truncate text-sm font-semibold text-fg">{stat.value}</dd>
        <dt class="truncate text-xs text-fg-3">{stat.label}</dt>
      </div>
    {/each}
  </dl>
{/if}

<div class="flex items-center gap-2 text-xs tracking-wide text-fg-3">
  <h3 class="text-xs font-normal tracking-wide uppercase">{t('subagents.steps')}</h3>
  {#if toolCalls > 0}<span class="tabular ml-auto">{tn('chat.subagents.toolCalls', toolCalls)}</span>{/if}
</div>
