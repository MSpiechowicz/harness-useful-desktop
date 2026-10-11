<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';
  import X from '@lucide/svelte/icons/x';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { activeSubagentCount } from '$lib/chat/subagents';
  import { cx } from '$lib/utils/cx';

  import SessionName from './SessionName.svelte';
  import { rovingIndex } from '$lib/utils/roving';

  const sessions = $derived(agentSessions.sessions);
  const canCreate = $derived(Boolean(harness.active?.detection.installed && project.active));

  let tabs: (HTMLElement | undefined)[] = $state([]);
  /** The chat whose name is being edited in its tab. */
  let renaming = $state<string | null>(null);
  let closing = $state<ChatSession | null>(null);
  let confirmOpen = $state(false);

  /** Closing a chat that is still working loses the turn in progress, so it asks first. */
  function close(session: ChatSession): void {
    if (session.streaming) {
      closing = session;
      confirmOpen = true;

      return;
    }

    void agentSessions.stop(session.key);
  }

  function create(): void {
    if (harness.active && project.active) {
      void agentSessions.create(harness.active.info.id, project.active.path);
    }
  }

  function rename(session: ChatSession): void {
    if (!session.ended) {
      renaming = session.key;
    }
  }

  function onKeydown(event: KeyboardEvent, index: number): void {
    if (event.key === 'F2') {
      event.preventDefault();
      rename(sessions[index]);

      return;
    }

    const next = rovingIndex(event.key, index, sessions.length, 'horizontal');
    if (next !== null) {
      event.preventDefault();
      agentSessions.select(sessions[next].key);
      tabs[next]?.focus();
    }
  }
</script>

<div class="flex min-w-0 flex-1 items-center gap-1">
  <!-- Each tab marks its harness with a small dot in the harness colour (the name is in the tooltip and for screen readers). -->
  <div role="tablist" aria-label="Chats" class="flex min-w-0 items-center gap-1 overflow-x-auto py-0.5">
    {#each sessions as session, index (session.key)}
      {@const selected = session.key === agentSessions.activeKey}
      {@const summary = harness.get(session.info.harness)}
      {@const helpers = activeSubagentCount(session.chat)}
      <div
        class={cx(
          'group flex shrink-0 items-center rounded-md border pr-0.5 transition-colors',
          renaming === session.key ? 'edge-accent bg-surface-3 text-fg' : selected ? 'border-line-strong bg-surface-3 text-fg shadow-raised' : 'border-transparent text-fg-2 hover:bg-hover active:bg-press',
        )}
      >
        <button
          bind:this={tabs[index]}
          type="button"
          role="tab"
          aria-selected={selected}
          tabindex={selected ? 0 : -1}
          title="{agentSessions.title(session.key)}{summary ? ` · ${summary.info.name}` : ''} · double-click to rename"
          class="flex h-7 max-w-52 items-center gap-2 pr-1 pl-2.5 text-sm"
          onclick={() => agentSessions.select(session.key)}
          ondblclick={() => rename(session)}
          onkeydown={(event) => onKeydown(event, index)}
        >
          {#if summary}<span class="size-2 shrink-0 rounded-full" style:background="var(--harness-{summary.info.id})" aria-hidden="true"></span><span class="sr-only">{summary.info.name}: </span>{/if}
          {#if renaming !== session.key}<span class="truncate">{agentSessions.title(session.key)}</span>{/if}
          {#if session.streaming || helpers > 0}
            <Spinner size={16} class="text-accent-text" label="Working" />
            {#if helpers > 0}<span class="tabular -ml-1 text-xs text-accent-text" title="{helpers} subagent{helpers === 1 ? '' : 's'} running">{helpers}<span class="sr-only">{` subagent${helpers === 1 ? '' : 's'} running`}</span></span>{/if}
          {:else if session.ended}
            <StatusDot tone="danger" />
            <span class="sr-only">Stopped</span>
          {/if}
        </button>
        {#if renaming === session.key}
          <SessionName
            {session}
            ondone={() => {
              renaming = null;
              tabs[index]?.focus();
            }}
          />
        {/if}
        <IconButton icon={X} label="Close chat" size="sm" onclick={() => close(session)} />
      </div>
    {/each}
    {#each agentSessions.starting as chat (chat.key)}
      {@const selected = chat.key === agentSessions.activeKey}
      {@const summary = harness.get(chat.harness)}
      <!-- A chat whose agent is still starting: the tab is there at once, the session takes it over when ready. -->
      <div class={cx('flex shrink-0 items-center rounded-md border pr-2 transition-colors', selected ? 'border-line-strong bg-surface-3 text-fg shadow-raised' : 'border-transparent text-fg-2 hover:bg-hover active:bg-press')}>
        <button
          type="button"
          role="tab"
          aria-selected={selected}
          tabindex={selected ? 0 : -1}
          class="flex h-7 max-w-52 items-center gap-2 pr-1 pl-2.5 text-sm"
          onclick={() => agentSessions.select(chat.key)}
        >
          {#if summary}<span class="size-2 shrink-0 rounded-full" style:background="var(--harness-{summary.info.id})" aria-hidden="true"></span><span class="sr-only">{summary.info.name}: </span>{/if}
          <span class="truncate">{agentSessions.title(chat.key)}</span>
          <Spinner size={16} class="text-accent-text" label="Starting" />
        </button>
      </div>
    {/each}
  </div>

  <IconButton icon={Plus} label={canCreate ? 'Open another chat' : 'Open a project and install a harness to start a chat'} size="sm" disabled={!canCreate} onclick={create} />
</div>

<ConfirmDialog
  bind:open={confirmOpen}
  title="Stop this chat?"
  description="{closing ? agentSessions.title(closing.key) : 'This chat'} is still working. Closing it stops the agent and discards the answer in progress."
  confirmLabel="Stop chat"
  cancelLabel="Keep working"
  onconfirm={() => closing && void agentSessions.stop(closing.key)}
/>
