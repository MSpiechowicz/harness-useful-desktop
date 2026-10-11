<script lang="ts">
  import Ban from '@lucide/svelte/icons/ban';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import Clock from '@lucide/svelte/icons/clock';
  import { slide } from 'svelte/transition';

  import Spinner from '$lib/components/Spinner.svelte';
  import { formatTokens, formatUsd } from '$lib/dashboard/format';
  import { scaled } from '$lib/design/motion';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import type { SubagentStatus } from '$lib/ipc/bindings/SubagentStatus';
  import { cx } from '$lib/utils/cx';

  interface Props {
    subagents: Subagent[];
  }

  let { subagents }: Props = $props();

  let openId = $state<string | null>(null);

  const STATUS_LABEL: Record<SubagentStatus, string> = { pending: 'Waiting', running: 'Running', completed: 'Done', failed: 'Failed', aborted: 'Stopped' };

  const counts = $derived.by(() => {
    const running = subagents.filter((subagent) => subagent.status === 'running' || subagent.status === 'pending').length;
    const failed = subagents.filter((subagent) => subagent.status === 'failed').length;
    const done = subagents.length - running - failed;

    return [running && `${running} running`, done && `${done} done`, failed && `${failed} failed`].filter(Boolean).join(', ');
  });
  const totalCost = $derived(subagents.reduce((sum, subagent) => sum + (subagent.costUsd ?? 0), 0));
  const totalTokens = $derived(subagents.reduce((sum, subagent) => sum + (subagent.tokens ?? 0), 0));

  /** What a subagent is doing now, in one line. */
  function activity(subagent: Subagent): string {
    if (subagent.status === 'pending') {
      return 'Waiting to start';
    }
    if (subagent.status !== 'running') {
      // A finished one shows what it found last; the assignment is one click away.
      return subagent.recentOutput?.at(-1) ?? subagent.description ?? subagent.task ?? STATUS_LABEL[subagent.status];
    }

    return subagent.intent ?? (subagent.currentTool ? `${subagent.currentTool} ${subagent.currentArgs ?? ''}`.trim() : (subagent.description ?? 'Working'));
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
<section aria-label="Subagents" class="border-t border-line">
  <header class="flex items-center gap-2 px-4 pt-2.5 pb-1.5 text-xs text-fg-3">
    <span class="font-medium text-fg-2">Subagents</span>
    <span>· {counts}</span>
    <span class="tabular ml-auto">{formatTokens(totalTokens)} tokens{totalCost > 0 ? ` · ${formatUsd(totalCost)}` : ''}</span>
  </header>

  <ul class="flex flex-col pb-1.5">
    {#each subagents as subagent (subagent.id)}
      {@const expanded = openId === subagent.id}
      <li>
        <button
          type="button"
          aria-expanded={expanded}
          class="flex w-full items-center gap-3 px-4 py-1.5 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press"
          onclick={() => (openId = expanded ? null : subagent.id)}
        >
          <span class="grid size-4 shrink-0 place-items-center" title={STATUS_LABEL[subagent.status]}>
            {#if subagent.status === 'running'}
              <Spinner size={14} class="text-accent-text" label="Running" />
            {:else if subagent.status === 'completed'}
              <CircleCheck size={14} class="text-success" aria-label="Done" />
            {:else if subagent.status === 'failed'}
              <CircleX size={14} class="text-danger" aria-label="Failed" />
            {:else if subagent.status === 'aborted'}
              <Ban size={14} class="text-fg-3" aria-label="Stopped" />
            {:else}
              <Clock size={14} class="text-fg-3" aria-label="Waiting" />
            {/if}
          </span>

          <span class="flex min-w-0 flex-1 items-baseline gap-2">
            <span class="shrink-0 text-sm font-medium text-fg">{subagent.id}</span>
            <span class="shrink-0 rounded-sm bg-surface-3 px-1.5 text-xs text-fg-2">{subagent.agent}</span>
            <span class={cx('min-w-0 truncate text-sm', subagent.status === 'running' ? 'text-fg-2' : 'text-fg-3')}>{activity(subagent)}</span>
          </span>

          <span class="tabular flex shrink-0 items-center gap-3 text-xs text-fg-3">
            {#if subagent.tokens !== null}<span>{formatTokens(subagent.tokens)} tok</span>{/if}
            {#if subagent.durationMs !== null}<span class="w-12 text-right">{elapsed(subagent.durationMs)}</span>{/if}
          </span>
          <ChevronRight size={14} aria-hidden="true" class={cx('shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)]', expanded && 'rotate-90')} />
        </button>

        {#if expanded}
          <div transition:slide={{ duration: scaled(160) }} class="flex flex-col gap-2.5 overflow-hidden px-4 pt-1 pb-3 pl-11 text-sm">
            {#if subagent.task}<p class="whitespace-pre-wrap text-fg-2">{subagent.task}</p>{/if}

            {#if subagent.recentTools && subagent.recentTools.length > 0}
              <ul class="flex flex-col gap-0.5 font-mono text-xs" aria-label="Recent tools">
                {#each subagent.recentTools as tool, index (index)}
                  <li class={cx('truncate', tool.isError ? 'text-danger' : 'text-fg-3')}><span class="text-fg-2">{tool.tool}</span> {tool.args}</li>
                {/each}
              </ul>
            {/if}

            {#if subagent.recentOutput && subagent.recentOutput.length > 0}
              <pre class="max-h-40 overflow-auto rounded-md border border-line bg-canvas px-3 py-2 font-mono text-xs leading-relaxed whitespace-pre-wrap text-fg-2" aria-label="Recent output">{subagent.recentOutput.join('\n')}</pre>
            {/if}

            <p class="tabular flex flex-wrap gap-x-3 text-xs text-fg-3">
              {#if subagent.model}<span>{subagent.model}</span>{/if}
              {#if subagent.toolCount !== null}<span>{subagent.toolCount} tool calls</span>{/if}
              {#if subagent.contextTokens !== null && subagent.contextWindow}<span>context {formatTokens(subagent.contextTokens)} / {formatTokens(subagent.contextWindow)}</span>{/if}
              {#if subagent.costUsd !== null}<span>{formatUsd(subagent.costUsd)}</span>{/if}
              {#if subagent.detached}<span>runs in the background</span>{/if}
            </p>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
</section>
