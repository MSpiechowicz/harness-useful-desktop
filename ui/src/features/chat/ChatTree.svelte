<script lang="ts">
  import X from '@lucide/svelte/icons/x';
  import { fade, slide } from 'svelte/transition';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import HarnessChatIcon from '$lib/components/HarnessChatIcon.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { scaled } from '$lib/design/motion';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { activeSubagentCount } from '$lib/chat/subagents';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import SessionName from './SessionName.svelte';

  const sessions = $derived(agentSessions.sessions);

  let rows: (HTMLElement | undefined)[] = $state([]);
  /** The chat whose name is being edited on its card. */
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

  function show(key: string): void {
    agentSessions.select(key);
    nav.go('chat');
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

    const next = rovingIndex(event.key, index, sessions.length, 'vertical');
    if (next !== null && !['Home', 'End'].includes(event.key)) {
      event.preventDefault();
      rows[next]?.focus();
    }
  }
</script>

<!-- The open chats under the Chat destination, hung on a line from its icon: rows the size and style of the menu's own
     (28px, no border, the same tint when open), each with its harness dot, name and what it is doing. What a chat is
     about is in its tooltip. -->
{#snippet branch(last: boolean)}
  <span aria-hidden="true" class={cx('absolute top-0 left-0 w-px bg-line-strong', last ? 'h-[14px]' : 'h-full')}></span>
  <span aria-hidden="true" class="absolute top-[14px] left-0 h-px w-[10px] bg-line-strong"></span>
{/snippet}

<div role="group" aria-label={t('chat.tabs.label')} transition:slide={{ duration: scaled(200) }} class="flex flex-col pt-[2px] pb-[4px] pl-[20px]">
  {#each sessions as session, index (session.key)}
    {@const selected = session.key === agentSessions.activeKey && nav.current === 'chat'}
    {@const summary = harness.get(session.info.harness)}
    {@const helpers = activeSubagentCount(session.chat)}
    {@const description = agentSessions.description(session.key)}
    {@const title = agentSessions.title(session.key)}
    <div in:fade={{ duration: scaled(160) }} class="group/card relative pl-[14px]">
      {@render branch(index === sessions.length - 1 && agentSessions.starting.length === 0)}
      <div class={cx('relative flex h-[28px] items-center rounded-sm transition-colors', renaming === session.key ? 'bg-surface-3 text-fg' : selected ? 'bg-accent-wash text-accent-text' : 'text-fg-2 hover:bg-hover hover:text-fg')}>
        <button
          bind:this={rows[index]}
          type="button"
          aria-current={selected ? 'true' : undefined}
          title={[summary ? t('chat.tabs.tipHarness', { title, harness: summary.info.name }) : t('chat.tabs.tip', { title }), description].filter(Boolean).join('\n')}
          class="flex h-full min-w-0 flex-1 items-center gap-[8px] pr-[26px] pl-[8px] text-left text-base"
          onclick={() => show(session.key)}
          ondblclick={() => rename(session)}
          onkeydown={(event) => onKeydown(event, index)}
        >
          {#if summary}<HarnessChatIcon harness={summary.info.id} class="shrink-0" /><span class="sr-only">{t('chat.tabs.harness', { name: summary.info.name })}</span>{/if}
          {#if renaming !== session.key}<span class={cx('min-w-0 truncate', selected && 'font-medium')}>{title}</span>{/if}
          {#if session.streaming || helpers > 0}
            <Spinner size={14} class="shrink-0 text-accent-text" label={t('chat.tabs.working')} />
            {#if helpers > 0}<span class="tabular -ml-1 text-xs text-accent-text" title={tn('chat.tabs.subagents', helpers)}><span aria-hidden="true">{helpers}</span><span class="sr-only">{tn('chat.tabs.subagents', helpers)}</span></span>{/if}
          {:else if session.ended}
            <StatusDot tone="danger" />
            <span class="sr-only">{t('chat.tabs.stopped')}</span>
          {/if}
        </button>
        {#if renaming === session.key}
          <SessionName
            {session}
            ondone={() => {
              renaming = null;
              rows[index]?.focus();
            }}
          />
        {/if}
        <button
          type="button"
          aria-label={t('chat.tabs.close')}
          title={t('chat.tabs.close')}
          class="absolute top-[3px] right-[3px] grid size-[22px] place-items-center rounded-sm text-fg-3 opacity-0 transition-opacity hover:bg-hover hover:text-fg focus-visible:opacity-100 group-focus-within/card:opacity-100 group-hover/card:opacity-100"
          onclick={() => close(session)}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
    </div>
  {/each}
  {#each agentSessions.starting as chat, index (chat.key)}
    {@const selected = chat.key === agentSessions.activeKey && nav.current === 'chat'}
    {@const summary = harness.get(chat.harness)}
    <!-- A chat whose agent is still starting: the row is there at once, the session takes it over when ready. -->
    <div class="relative pl-[14px]">
      {@render branch(index === agentSessions.starting.length - 1)}
      <button
        type="button"
        aria-current={selected ? 'true' : undefined}
        class={cx('flex h-[28px] w-full items-center gap-[8px] rounded-sm px-[8px] text-left text-base transition-colors', selected ? 'bg-accent-wash text-accent-text' : 'text-fg-2 hover:bg-hover hover:text-fg')}
        onclick={() => show(chat.key)}
      >
        {#if summary}<HarnessChatIcon harness={summary.info.id} class="shrink-0" /><span class="sr-only">{t('chat.tabs.harness', { name: summary.info.name })}</span>{/if}
        <span class="min-w-0 truncate">{agentSessions.title(chat.key)}</span>
        <Spinner size={14} class="shrink-0 text-accent-text" label={t('chat.tabs.starting')} />
      </button>
    </div>
  {/each}
</div>

<ConfirmDialog
  bind:open={confirmOpen}
  title={t('chat.tabs.stop.title')}
  description={t('chat.tabs.stop.description', { name: closing ? agentSessions.title(closing.key) : t('chat.tabs.stop.thisChat') })}
  confirmLabel={t('chat.tabs.stop.confirm')}
  cancelLabel={t('chat.tabs.stop.cancel')}
  onconfirm={() => closing && void agentSessions.stop(closing.key)}
/>
