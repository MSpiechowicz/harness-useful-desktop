<script lang="ts">
  import Ban from '@lucide/svelte/icons/ban';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import Clock from '@lucide/svelte/icons/clock';

  import Spinner from '$lib/components/Spinner.svelte';
  import { activity, elapsed, statusLabel } from '$lib/chat/subagents';
  import { formatTokens, formatUsd } from '$lib/dashboard/format';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import { subagentPanel } from '$lib/stores/subagentPanel.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    subagents: Subagent[];
    /** The chat the rows belong to: a click opens that chat's subagent panel on the row. Without one the rows only list. */
    panelKey?: string;
    /** Pinned above the composer as its own card, rows scrolling inside a capped height, instead of inline in a tool card. */
    docked?: boolean;
  }

  let { subagents, panelKey, docked = false }: Props = $props();

  const selectedId = $derived(panelKey === undefined ? null : subagentPanel.selectedId(panelKey));

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
</script>

<!-- The subagents a `task` call spawned: one live row each. A click opens the subagent panel with what it was asked and what it did. -->
<section aria-label={t('chat.subagents.label')} class={docked ? 'chat-radius mb-2 overflow-hidden border-card border-line bg-surface-1 bg-clip-padding' : 'border-t border-line'}>
  <header class="flex items-center gap-2 px-4 pt-2.5 pb-1.5 text-xs text-fg-3">
    <span class="font-medium text-fg-2">{t('chat.subagents.label')}</span>
    <span>· {counts}</span>
    <span class="tabular ml-auto">{t('chat.subagents.totalTokens', { tokens: formatTokens(totalTokens) })}{totalCost > 0 ? ` · ${formatUsd(totalCost)}` : ''}</span>
  </header>

  <!-- px: the rows' hover is a rounded box inset from the card edge, like the other chat rows; inset + row padding keep the text at 1rem. -->
  <ul class={cx('flex flex-col gap-0.5 px-1.5 pb-1.5', docked && 'max-h-64 overflow-y-auto')}>
    {#each subagents as subagent (subagent.id)}
      {@const selected = selectedId === subagent.id}
      <li>
        <button
          type="button"
          aria-pressed={selected}
          class={cx(
            'group relative flex w-full items-center gap-3 rounded-sm px-2.5 py-1.5 text-left transition-colors duration-[var(--dur-fast)]',
            selected ? 'bg-accent-wash' : 'hover:bg-hover active:bg-press',
          )}
          data-flash={panelKey !== undefined && subagentPanel.flashesRow(panelKey, subagent.id) ? '' : undefined}
          onclick={() => panelKey !== undefined && subagentPanel.open(panelKey, subagent.id)}
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
          <ChevronRight size={14} aria-hidden="true" class={cx('shrink-0 transition-colors duration-[var(--dur-fast)] group-hover:text-fg', selected ? 'text-fg' : 'text-fg-3')} />
        </button>
      </li>
    {/each}
  </ul>
</section>
