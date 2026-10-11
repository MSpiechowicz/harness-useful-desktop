<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import Clock from '@lucide/svelte/icons/clock';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import CodeBlock from '$lib/components/CodeBlock.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { changesFiles, commandOf, primaryArg, TOOL_ICONS, toolKind, toolTitle } from '$lib/chat/tool-meta';
  import { spawnedBy } from '$lib/chat/subagents';
  import { toolContext } from '$lib/chat/tool-context';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import type { ToolCallState, ToolStatus } from '$lib/chat/types';
  import { scaled } from '$lib/design/motion';
  import { git } from '$lib/stores/git.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

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
  const spawned = $derived(kind === 'task' ? spawnedBy(subagents, call.id) : []);
  const argsJson = $derived(call.args === null ? '' : JSON.stringify(call.args, null, 2));

  const STATUS_LABEL: Record<ToolStatus, string> = { pending: 'Waiting', running: 'Running', ok: 'Done', error: 'Failed' };
  const label = $derived(call.status === 'pending' && !active ? 'Not run' : STATUS_LABEL[call.status]);

  /** Opens Changes limited to the files this call touched, in the project its chat belongs to. */
  function viewChanges(): void {
    const target = where?.project ?? project.active?.path;
    if (target) {
      git.showPaths(target, call.touchedPaths, where?.label ?? 'this chat');
    }
  }
</script>

<div class={cx('rounded-lg border bg-surface-1 bg-clip-padding', call.status === 'error' ? 'border-danger-edge' : 'border-line')} data-tool-status={call.status}>
  <button
    type="button"
    aria-expanded={open}
    class="flex w-full items-center gap-3 rounded-lg px-4 py-2.5 text-left transition-colors hover:bg-hover active:bg-press"
    onclick={() => (open = !open)}
  >
    <Icon size={16} class="shrink-0 text-fg-2" aria-hidden="true" />

    <span class="min-w-0 flex-1 truncate text-sm">
      <span class="font-medium text-fg">{toolTitle(call)}</span>
      {#if summary}<span class="ml-2 font-mono text-xs text-fg-3">{summary}</span>{/if}
    </span>

    <span class="inline-flex shrink-0 items-center gap-1.5 text-xs text-fg-3">
      {#if call.status === 'running'}
        <Spinner size={16} class="text-accent-text" />
      {:else if call.status === 'ok'}
        <CircleCheck size={16} class="text-success" aria-hidden="true" />
      {:else if call.status === 'error'}
        <CircleX size={16} class="text-danger" aria-hidden="true" />
      {:else}
        <Clock size={16} aria-hidden="true" />
      {/if}
      <span>{label}</span>
    </span>

    <ChevronRight size={16} aria-hidden="true" class={cx('shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} />
  </button>

  {#if call.status === 'running' && call.partial && !open}
    <p class="truncate px-4 pb-2.5 font-mono text-xs text-fg-3">{call.partial.trimEnd().split('\n').at(-1)}</p>
  {/if}

  {#if spawned.length > 0}
    <SubagentList subagents={spawned} />
  {/if}

  {#if open}
    <div transition:slide={{ duration: scaled(180) }} class="flex flex-col gap-4 overflow-hidden border-t border-line px-4 pt-4 pb-4">
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
          <Button size="sm" variant="ghost" onclick={viewChanges}>View changes</Button>
        </div>
      {/if}
    </div>
  {/if}
</div>
