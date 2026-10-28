<script lang="ts">
  import CircleX from '@lucide/svelte/icons/circle-x';
  import type { Snippet } from 'svelte';

  import Spinner from '$lib/components/Spinner.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import type { ToolCallState } from '$lib/chat/types';
  import { t } from '$lib/i18n/i18n.svelte';

  import ChatRow from './ChatRow.svelte';
  import StatusPill from './StatusPill.svelte';
  import ToolResult from './ToolResult.svelte';

  interface Props {
    call: ToolCallState;
    /** Whether the agent is still working; a call that never started is "waiting" then, "not run" after. */
    active: boolean;
    icon: IconComponent;
    /** The title line. */
    heading: string;
    /** Beside it in mono: the query, the first note's title, what was rated. */
    hint?: string | null;
    /** What the pill says once the call finished well; none when that is not known (the pill is left out). */
    done: { tone: 'info' | 'idle'; label: string } | null;
    /** The pill while the call runs. */
    runningLabel?: string;
    /** The body of a call that finished well. A call that runs or failed shows its output instead. */
    content: Snippet;
  }

  let { call, active, icon: Icon, heading, hint = null, done, runningLabel, content }: Props = $props();

  let open = $state(false);

  const failed = $derived(call.status === 'error');
  const running = $derived(call.status === 'running' || (call.status === 'pending' && active));

  const pill = $derived.by((): { tone: 'run' | 'error' | 'idle' | 'info'; label: string } | null => {
    if (running) {
      return { tone: 'run', label: runningLabel ?? t('chat.tool.status.running') };
    }
    if (failed) {
      return { tone: 'error', label: t('chat.tool.status.error') };
    }
    if (call.status === 'pending') {
      return { tone: 'idle', label: t('chat.tool.notRun') };
    }

    return done;
  });
</script>

{#snippet spinner()}<Spinner size={16} />{/snippet}
{#snippet crossed()}<CircleX size={16} aria-hidden="true" />{/snippet}

<!-- The shape of every project-memory card: the info tile (the run tile while the call runs), the title with its mono hint, a
     pill with no icon once it finished (the memory cards do not say "Done"), and the output of a call that runs or failed in place of the body. -->
<ChatRow {open} ontoggle={() => (open = !open)} tone={running ? 'run' : 'info'} {failed} data-tool-status={call.status}>
  {#snippet icon()}
    {#if running}<Spinner size={16} />{:else}<Icon size={16} aria-hidden="true" />{/if}
  {/snippet}

  {#snippet title()}
    <span class="font-medium text-fg">{heading}</span>
    {#if hint}<span class="ml-2 font-mono text-xs text-fg-3">{hint}</span>{/if}
  {/snippet}

  {#snippet status()}
    {#if pill}<StatusPill tone={pill.tone} label={pill.label} icon={running ? spinner : failed ? crossed : undefined} />{/if}
  {/snippet}

  {#snippet body()}
    {#if call.status === 'ok'}
      {@render content()}
    {:else}
      <div class="px-4 py-4">
        <ToolResult text={running ? call.partial : call.resultText} {running} {failed} />
      </div>
    {/if}
  {/snippet}
</ChatRow>
