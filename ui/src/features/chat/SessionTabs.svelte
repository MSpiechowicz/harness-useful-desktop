<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';
  import X from '@lucide/svelte/icons/x';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  const sessions = $derived(agentSessions.sessions);
  const canCreate = $derived(Boolean(harness.active?.detection.installed && project.active));

  let tabs: (HTMLElement | undefined)[] = $state([]);
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

  function onKeydown(event: KeyboardEvent, index: number): void {
    const next = rovingIndex(event.key, index, sessions.length, 'horizontal');
    if (next !== null) {
      event.preventDefault();
      agentSessions.select(sessions[next].key);
      tabs[next]?.focus();
    }
  }
</script>

<div class="flex min-w-0 flex-1 items-center gap-1">
  <div role="tablist" aria-label="Chats" class="flex min-w-0 items-center gap-1 overflow-x-auto py-0.5">
    {#each sessions as session, index (session.key)}
      {@const selected = session.key === agentSessions.activeKey}
      {@const summary = harness.get(session.info.harness)}
      <div class={cx('group flex shrink-0 items-center rounded-md border pr-0.5 transition-colors', selected ? 'border-line-strong bg-surface-3 text-fg shadow-raised' : 'border-transparent text-fg-2 hover:bg-hover active:bg-press')}>
        <button
          bind:this={tabs[index]}
          type="button"
          role="tab"
          aria-selected={selected}
          tabindex={selected ? 0 : -1}
          title={session.info.project}
          class="flex h-7 max-w-52 items-center gap-2 pr-1 pl-2.5 text-sm"
          onclick={() => agentSessions.select(session.key)}
          onkeydown={(event) => onKeydown(event, index)}
        >
          {#if summary}<HarnessBadge info={summary.info} size="sm" />{/if}
          <span class="truncate">{session.name ?? 'New chat'}</span>
          {#if session.streaming}
            <Spinner size={16} class="text-accent-text" label="Working" />
          {:else if session.ended}
            <StatusDot tone="danger" />
            <span class="sr-only">Stopped</span>
          {/if}
        </button>
        <IconButton icon={X} label="Close chat" size="sm" onclick={() => close(session)} />
      </div>
    {/each}
  </div>

  <IconButton icon={Plus} label={canCreate ? 'Open another chat' : 'Open a project and install a harness to start a chat'} size="sm" disabled={!canCreate} onclick={create} />
</div>

<ConfirmDialog
  bind:open={confirmOpen}
  title="Stop this chat?"
  description="{closing?.name ?? 'This chat'} is still working. Closing it stops the agent and discards the answer in progress."
  confirmLabel="Stop chat"
  cancelLabel="Keep working"
  onconfirm={() => closing && void agentSessions.stop(closing.key)}
/>
