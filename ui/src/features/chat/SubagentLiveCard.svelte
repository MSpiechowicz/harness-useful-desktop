<script lang="ts">
  import Ban from '@lucide/svelte/icons/ban';
  import Clock from '@lucide/svelte/icons/clock';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Square from '@lucide/svelte/icons/square';
  import X from '@lucide/svelte/icons/x';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import IconTile from '$lib/components/IconTile.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { humanToolName, TOOL_ICONS, toolKind } from '$lib/chat/tool-meta';
  import type { CurrentAction } from '$lib/chat/subagent-steps';
  import { activity, elapsed, isSubagentActive, reportTail, statusLabel } from '$lib/chat/subagents';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import { cx } from '$lib/utils/cx';

  interface Props {
    subagent: Subagent;
    /** What it is doing now, from its steps; null when nothing says. */
    action: CurrentAction | null;
    /** How long it has run, in ms; null while unknown. */
    ms: number | null;
    toolCalls: number | null;
    stopping: boolean;
    onstop: () => void;
  }

  let { subagent, action, ms, toolCalls, stopping, onstop }: Props = $props();

  const running = $derived(subagent.status === 'running');
  const canStop = $derived(running && subagent.canStop === true);
  const TONES = { running: 'run', completed: 'success', failed: 'danger', pending: 'plain', aborted: 'plain' } as const;

  // What the second line says: the tool it runs (name and arguments), its own prose, or for a finished one what it ended with.
  const doing = $derived.by(() => {
    if (running) {
      return action ?? { name: null, detail: activity(subagent) };
    }

    return { name: null, detail: reportTail(subagent.report) ?? activity(subagent) };
  });
  const Icon = $derived(running && doing.name ? TOOL_ICONS[toolKind(doing.name)] : subagent.status === 'failed' ? X : subagent.status === 'aborted' ? Ban : subagent.status === 'pending' ? Clock : MessageSquare);

  // The check draws itself once, when the subagent finishes while the card is on screen. One that was done on arrival shows it drawn.
  let wasActive = untrack(() => isSubagentActive(subagent));
  let drawCheck = $state(false);

  $effect(() => {
    const active = isSubagentActive(subagent);

    if (wasActive && subagent.status === 'completed') {
      drawCheck = true;
    }

    wasActive = active;
  });

  // A changed line rolls up. The one the card opened with does not.
  const lineKey = $derived(`${doing.name ?? ''}\u0000${doing.detail}`);
  const firstKey = untrack(() => lineKey);
  const rolls = $derived(lineKey !== firstKey);
</script>

<div class="panel-pad pt-[var(--panel-block-gap)]">
  <div
    class={cx(
      'flex items-center gap-3.5 rounded-md border bg-surface-2 py-2.5 pr-2.5 pl-3.5 transition-colors duration-[var(--dur-slow)]',
      running ? 'border-accent/30' : subagent.status === 'completed' ? 'border-success/30' : 'border-line',
    )}
    data-testid="subagent-live"
    data-status={subagent.status}
  >
    <IconTile size="lg" tone={TONES[subagent.status]}>
      {#if subagent.status === 'completed'}
        <svg class={cx('size-4', drawCheck && 'check-draw')} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path pathLength="1" d="M20 6 9 17l-5-5" />
        </svg>
      {:else}
        <Icon size={16} aria-hidden="true" />
      {/if}
    </IconTile>

    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <div class="flex items-center gap-1.5 text-xs text-fg-3">
        {#if running}<Spinner size={12} class="text-accent-text" />{/if}
        <span class={cx('font-semibold', running ? 'text-fg' : subagent.status === 'completed' ? 'text-success' : subagent.status === 'failed' ? 'text-danger' : 'text-fg-2')}>{statusLabel(subagent.status)}</span>
        {#if ms !== null}<span class="tabular">{elapsed(ms)}</span>{/if}
        {#if toolCalls !== null && toolCalls > 0}
          <span aria-hidden="true">·</span>
          <span class="tabular">{tn('chat.subagents.toolCalls', toolCalls)}</span>
        {/if}
      </div>

      {#key lineKey}
        <div class={cx('truncate text-sm leading-[var(--detail-line)] text-fg', rolls && 'roll-up')} title={doing.name ? `${humanToolName(doing.name)} ${doing.detail}` : doing.detail}>
          {#if doing.name}<b class="font-medium">{humanToolName(doing.name)}</b> <span class="font-mono text-xs text-fg-2">{doing.detail}</span>{:else}{doing.detail}{/if}
        </div>
      {/key}
    </div>

    {#if canStop}
      <Button variant="danger" loading={stopping} title={t('subagents.stop.hint')} onclick={onstop}>
        {#snippet leading()}<Square size={14} fill="currentColor" aria-hidden="true" />{/snippet}
        {t('subagents.stop')}
      </Button>
    {/if}
  </div>
</div>
