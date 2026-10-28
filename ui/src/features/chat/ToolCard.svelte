<script lang="ts">
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import Clock from '@lucide/svelte/icons/clock';

  import Button from '$lib/components/Button.svelte';
  import CodeBlock from '$lib/components/CodeBlock.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { changesFiles, commandOf, primaryArg, TOOL_ICONS, toolKind, toolTitle } from '$lib/chat/tool-meta';
  import { isDockedCall, spawnedBy } from '$lib/chat/subagents';
  import { toolContext } from '$lib/chat/tool-context';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import type { ToolCallState } from '$lib/chat/types';
  import { git } from '$lib/stores/git.svelte';
  import { isJsonObject } from '$lib/chat/tools';
  import { project } from '$lib/stores/project.svelte';
  import { subagentPanel } from '$lib/stores/subagentPanel.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ChatRow from './ChatRow.svelte';
  import StatusPill from './StatusPill.svelte';
  import SubagentList from './SubagentList.svelte';
  import TerminalBlock from './TerminalBlock.svelte';
  import ToolResult from './ToolResult.svelte';

  interface Props {
    call: ToolCallState;
    /** Whether the agent is still working; a tool that never started is "waiting" then, "not run" after. */
    active: boolean;
    /** The session's subagents; a `task` call lists the ones it spawned. */
    subagents?: Record<string, Subagent>;
  }

  let { call, active, subagents = {} }: Props = $props();

  let open = $state(false);

  const where = toolContext();

  const kind = $derived(toolKind(call.name));
  const Icon = $derived(TOOL_ICONS[kind]);
  const summary = $derived(primaryArg(call.args));
  const command = $derived(commandOf(call));
  // A `task` call lists what it spawned, but only once none of its subagents runs: while one is active the docked list
  // above the composer shows that call's rows, and repeating them in the card doubles the rows.
  const docked = $derived(isDockedCall(subagents, call.id));
  const spawned = $derived(kind === 'task' && !docked ? spawnedBy(subagents, call.id) : []);
  const argsJson = $derived(call.args === null || (isJsonObject(call.args) && Object.keys(call.args).length === 0) ? '' : JSON.stringify(call.args, null, 2));

  const label = $derived(call.status === 'pending' && !active ? t('chat.tool.notRun') : t(`chat.tool.status.${call.status}`));

  /** Opens Changes limited to the files this call touched, in the project its chat belongs to. */
  function viewChanges(): void {
    const target = where?.project ?? project.active?.path;
    if (target) {
      git.showPaths(target, call.touchedPaths, where?.label ?? t('chat.list.thisChat'));
    }
  }
</script>

<ChatRow
  {open}
  ontoggle={() => (open = !open)}
  tone={call.status === 'running' ? 'run' : 'plain'}
  failed={call.status === 'error'}
  data-tool-status={call.status}
  data-tool-call-id={call.id}
  data-flash={where?.sessionKey !== undefined && subagentPanel.flashesCall(where.sessionKey, call.id) ? '' : undefined}
>
  {#snippet icon()}<Icon size={16} aria-hidden="true" />{/snippet}

  {#snippet title()}
    <span class="font-medium text-fg">{toolTitle(call)}</span>
    {#if summary}<span class="ml-2 font-mono text-xs text-fg-3">{summary}</span>{/if}
  {/snippet}

  {#snippet status()}
    <StatusPill tone={call.status === 'running' ? 'run' : call.status === 'ok' ? 'ok' : call.status === 'error' ? 'error' : 'idle'} {label}>
      {#snippet icon()}
        {#if call.status === 'running'}
          <Spinner size={16} />
        {:else if call.status === 'ok'}
          <CircleCheck size={16} aria-hidden="true" />
        {:else if call.status === 'error'}
          <CircleX size={16} aria-hidden="true" />
        {:else}
          <Clock size={16} aria-hidden="true" />
        {/if}
      {/snippet}
    </StatusPill>
  {/snippet}

  {#snippet peek()}
    {#if call.status === 'running' && call.partial && !open}
      <p class="truncate px-4 pb-2.5 font-mono text-xs text-fg-3">{call.partial.trimEnd().split('\n').at(-1)}</p>
    {/if}
  {/snippet}

  {#snippet extra()}
    {#if spawned.length > 0}
      <SubagentList subagents={spawned} panelKey={where?.sessionKey} />
    {/if}
  {/snippet}

  {#snippet body()}
    <div class="flex flex-col gap-4 px-4 pt-4 pb-4">
      {#if command !== null}
        <TerminalBlock text="" {command} />
      {:else if argsJson !== ''}
        <CodeBlock code={argsJson} lang="json" />
      {/if}

      <ToolResult text={call.status === 'running' ? call.partial : call.resultText} running={call.status === 'running'} failed={call.status === 'error'} />

      {#if changesFiles(kind) && call.touchedPaths.length > 0}
        <div class="flex flex-wrap items-center gap-2">
          {#each call.touchedPaths as path (path)}
            <code class="rounded-sm bg-surface-3 px-2 py-0.5 font-mono text-xs text-fg-2">{path}</code>
          {/each}
          <Button variant="ghost" onclick={viewChanges}>{t('chat.tool.viewChanges')}</Button>
        </div>
      {/if}
    </div>
  {/snippet}
</ChatRow>
