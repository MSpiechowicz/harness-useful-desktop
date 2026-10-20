<script lang="ts">
  import Ban from '@lucide/svelte/icons/ban';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import Clock from '@lucide/svelte/icons/clock';
  import { slide } from 'svelte/transition';

  import Spinner from '$lib/components/Spinner.svelte';
  import { formatTokens, formatUsd } from '$lib/dashboard/format';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { scaled } from '$lib/design/motion';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import type { SubagentStatus } from '$lib/ipc/bindings/SubagentStatus';
  import { cx } from '$lib/utils/cx';

  interface Props {
    subagents: Subagent[];
    /** Pinned above the composer as its own card, rows scrolling inside a capped height, instead of inline in a tool card. */
    docked?: boolean;
  }

  let { subagents, docked = false }: Props = $props();

  let openId = $state<string | null>(null);

  function statusLabel(status: SubagentStatus): string {
    switch (status) {
      case 'pending':
        return t('chat.subagents.status.pending');

      case 'running':
        return t('chat.subagents.status.running');

      case 'completed':
        return t('chat.subagents.status.completed');

      case 'failed':
        return t('chat.subagents.status.failed');

      case 'aborted':
        return t('chat.subagents.status.aborted');
    }
  }

  const counts = $derived.by(() => {
    const running = subagents.filter((subagent) => subagent.status === 'running' || subagent.status === 'pending').length;
    const failed = subagents.filter((subagent) => subagent.status === 'failed').length;
    const done = subagents.length - running - failed;

    return [
      running && t('chat.subagents.running', { count: running }),
      done && t('chat.subagents.done', { count: done }),
      failed && t('chat.subagents.failed', { count: failed }),
    ]
      .filter(Boolean)
      .join(', ');
  });
  const totalCost = $derived(subagents.reduce((sum, subagent) => sum + (subagent.costUsd ?? 0), 0));
  const totalTokens = $derived(subagents.reduce((sum, subagent) => sum + (subagent.tokens ?? 0), 0));

  /** What a subagent is doing now, in one line. */
  function activity(subagent: Subagent): string {
    if (subagent.status === 'pending') {
      return t('chat.subagents.waitingToStart');
    }
    if (subagent.status !== 'running') {
      // A finished one shows what it found last; the assignment is one click away.
      return subagent.recentOutput?.at(-1) ?? subagent.description ?? subagent.task ?? statusLabel(subagent.status);
    }

    return subagent.intent ?? (subagent.currentTool ? `${subagent.currentTool} ${subagent.currentArgs ?? ''}`.trim() : (subagent.description ?? t('chat.subagents.working')));
  }

  function elapsed(ms: number | null): string | null {
    if (ms === null) {
      return null;
    }

    const seconds = Math.round(ms / 1000);

    return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, '0')}s`;
  }
</script>

<!-- The subagents a `task` call spawned: one live row each, expandable to what it was asked and what it did last. -->
<section aria-label={t('chat.subagents.label')} class={docked ? 'chat-radius mb-2 overflow-hidden border-card border-line bg-surface-1 bg-clip-padding' : 'border-t border-line'}>
  <!-- pr: the row's right padding + chevron (14px) + its gap, so the totals end where the rows' time column ends. -->
  <header class="flex items-center gap-2 pt-2.5 pr-[calc(1rem+14px+0.75rem)] pb-1.5 pl-4 text-xs text-fg-3">
    <span class="font-medium text-fg-2">{t('chat.subagents.label')}</span>
    <span>· {counts}</span>
    <span class="tabular ml-auto">{t('chat.subagents.totalTokens', { tokens: formatTokens(totalTokens) })}{totalCost > 0 ? ` · ${formatUsd(totalCost)}` : ''}</span>
  </header>

  <ul class={cx('flex flex-col pb-1.5', docked && 'max-h-64 overflow-y-auto')}>
    {#each subagents as subagent (subagent.id)}
      {@const expanded = openId === subagent.id}
      <li>
        <button
          type="button"
          aria-expanded={expanded}
          class="flex w-full items-center gap-3 px-4 py-1.5 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press"
          onclick={() => (openId = expanded ? null : subagent.id)}
        >
          <span class="grid size-4 shrink-0 place-items-center" title={statusLabel(subagent.status)}>
            {#if subagent.status === 'running'}
              <Spinner size={14} class="text-accent-text" label={t('chat.subagents.status.running')} />
            {:else if subagent.status === 'completed'}
              <CircleCheck size={14} class="text-success" aria-label={t('chat.subagents.status.completed')} />
            {:else if subagent.status === 'failed'}
              <CircleX size={14} class="text-danger" aria-label={t('chat.subagents.status.failed')} />
            {:else if subagent.status === 'aborted'}
              <Ban size={14} class="text-fg-3" aria-label={t('chat.subagents.status.aborted')} />
            {:else}
              <Clock size={14} class="text-fg-3" aria-label={t('chat.subagents.status.pending')} />
            {/if}
          </span>

          <span class="flex min-w-0 flex-1 items-baseline gap-2">
            <span class="shrink-0 text-sm font-medium text-fg">{subagent.id}</span>
            <span class="shrink-0 rounded-sm bg-surface-3 px-1.5 text-xs text-fg-2">{subagent.agent}</span>
            <span class={cx('min-w-0 truncate text-sm', subagent.status === 'running' ? 'text-fg-2' : 'text-fg-3')}>{activity(subagent)}</span>
          </span>

          <span class="tabular flex shrink-0 items-center gap-3 text-xs text-fg-3">
            <span class="w-24 text-right">{subagent.tokens !== null ? t('chat.subagents.totalTokens', { tokens: formatTokens(subagent.tokens) }) : ''}</span>
            <span class="w-14 text-right">{elapsed(subagent.durationMs) ?? ''}</span>
          </span>
          <ChevronRight size={14} aria-hidden="true" class={cx('shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)]', expanded && 'rotate-90')} />
        </button>

        {#if expanded}
          <div transition:slide={{ duration: scaled(160) }} class="flex flex-col gap-2.5 overflow-hidden px-4 pt-1 pb-3 pl-11 text-sm">
            {#if subagent.task}<p class="whitespace-pre-wrap text-fg-2">{subagent.task}</p>{/if}

            {#if subagent.recentTools && subagent.recentTools.length > 0}
              <ul class="flex flex-col gap-0.5 font-mono text-xs" aria-label={t('chat.subagents.recentTools')}>
                {#each subagent.recentTools as tool, index (index)}
                  <li class={cx('truncate', tool.isError ? 'text-danger' : 'text-fg-3')}><span class="text-fg-2">{tool.tool}</span> {tool.args}</li>
                {/each}
              </ul>
            {/if}

            {#if subagent.recentOutput && subagent.recentOutput.length > 0}
              <pre class="chat-radius max-h-40 overflow-auto border border-line bg-canvas px-3 py-2 font-mono text-xs leading-relaxed whitespace-pre-wrap text-fg-2" aria-label={t('chat.subagents.recentOutput')}>{subagent.recentOutput.join('\n')}</pre>
            {/if}

            <p class="tabular flex flex-wrap gap-x-3 text-xs text-fg-3">
              {#if subagent.model}<span>{subagent.model}</span>{/if}
              {#if subagent.toolCount !== null}<span>{tn('chat.subagents.toolCalls', subagent.toolCount)}</span>{/if}
              {#if subagent.contextTokens !== null && subagent.contextWindow}<span>{t('chat.subagents.context', { used: formatTokens(subagent.contextTokens), total: formatTokens(subagent.contextWindow) })}</span>{/if}
              {#if subagent.costUsd !== null}<span>{formatUsd(subagent.costUsd)}</span>{/if}
              {#if subagent.detached}<span>{t('chat.subagents.detached')}</span>{/if}
            </p>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
</section>
