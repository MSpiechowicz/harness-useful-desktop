<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import User from '@lucide/svelte/icons/user';

  import IconTile from '$lib/components/IconTile.svelte';
  import { stepClock } from '$lib/chat/subagent-time';
  import { humanToolName, TOOL_ICONS, toolKind } from '$lib/chat/tool-meta';
  import type { SubagentStepEntry } from '$lib/chat/types';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    entry: SubagentStepEntry;
    /** Whether this is the tool the subagent is running now: its tile is tinted and the row says so. */
    live?: boolean;
    /** When the subagent started, so a step's time reads as how far into the run it happened. */
    startedAt: number | null;
  }

  let { entry, live = false, startedAt }: Props = $props();

  const time = $derived(stepClock(entry.at, startedAt));
  // A tool call the harness gave no arguments for is a single line: its tile sits on that line, not on two.
  const oneLine = $derived(entry.kind === 'tool' && entry.detail.trim() === '');
</script>

<!-- One shape for every step: the tile, the name with the time on the first line, the detail under it. The tile is centred on
     the first two lines of text, so a message that wraps further leaves it where it is on every row. A step without a detail
     is one line, and its tile is centred on that line. -->
<li class={cx('grid grid-cols-[var(--tile-md)_minmax(0,1fr)] gap-x-3 py-1.5 text-sm leading-[var(--detail-line)]', oneLine ? 'items-center' : 'items-start')} data-step={entry.kind}>
  <IconTile size="md" tone={live || entry.kind === 'you' ? 'run' : 'plain'} class={cx(!oneLine && 'mt-[calc((2*var(--detail-line)-var(--tile-md))/2)]')}>
    {#if entry.kind === 'you'}
      <User size={16} aria-hidden="true" />
    {:else if entry.kind === 'tool'}
      {@const Icon = TOOL_ICONS[toolKind(entry.name ?? '')]}
      <Icon size={16} aria-hidden="true" />
    {:else}
      <MessageSquare size={16} aria-hidden="true" />
    {/if}
  </IconTile>

  <div class="min-w-0">
    <div class="flex items-baseline gap-2">
      <b class="font-medium text-fg">{entry.kind === 'you' ? t('subagents.step.you') : entry.kind === 'text' ? t('subagents.step.says') : humanToolName(entry.name ?? '')}</b>
      {#if live}<span class="text-xs text-accent-text">{t('subagents.step.running')}</span>{/if}
      {#if entry.kind === 'tool' && entry.status === 'failed'}<span class="text-xs text-danger">{t('subagents.step.failed')}</span>{/if}
      {#if time}<span class="tabular ml-auto text-xs text-fg-3">{time}</span>{/if}
    </div>

    {#if entry.kind === 'you'}
      <p class="break-words whitespace-pre-wrap text-fg-2">{entry.text}</p>
      {#if entry.route === 'orchestrator'}<p class="text-xs text-fg-3">{t('subagents.step.viaOrchestrator')}</p>{/if}
    {:else if entry.kind === 'tool' && !oneLine}
      <p class={cx('truncate font-mono text-xs leading-[var(--detail-line)]', entry.status === 'failed' ? 'text-danger' : 'text-fg-3')} title={entry.detail}>{entry.detail}</p>
    {:else if entry.kind === 'text'}
      <p class="break-words whitespace-pre-wrap text-fg-2">{entry.detail}</p>
    {/if}
  </div>
</li>
