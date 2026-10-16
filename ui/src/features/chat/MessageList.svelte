<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import History from '@lucide/svelte/icons/history';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import { cx } from '$lib/utils/cx';
  import { matchesShortcut } from '$lib/utils/shortcuts';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { provideToolContext } from '$lib/chat/tool-context';
  import type { TranscriptItem } from '$lib/chat/types';
  import { rise } from '$lib/design/motion';
  import { harness } from '$lib/stores/harness.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import MessageRow from './MessageRow.svelte';
  import WorkingIndicator from './WorkingIndicator.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  provideToolContext({ project: untrack(() => session.info.project), label: t('chat.list.thisChat') });

  const ESTIMATED_ROW_PX = 96;
  const ENTRANCE_MS = 240;

  let list: VirtualList<TranscriptItem> | undefined = $state();
  let atBottom = $state(true);
  let loadingEarlier = $state(false);
  let rechecking = $state(false);

  async function recheckModels(): Promise<void> {
    rechecking = true;
    await session.recheckModels();
    rechecking = false;
  }

  // Only what arrives while this list is open animates; scrolling rows back into view does not replay it.
  const firstNew = untrack(() => session.chat.nextOrder);
  const played = new Set<string>();

  function entrance(item: TranscriptItem): number {
    if (played.has(item.key)) {
      return 0;
    }

    played.add(item.key);

    return item.order >= firstNew ? ENTRANCE_MS : 0;
  }

  /** Ctrl/Cmd+End or Ctrl/Cmd+Down is the keyboard's "Jump to latest" (laptops without an End key); it works from the composer too, where plain End belongs to the text. */
  function onkeydown(event: KeyboardEvent): void {
    const jump = matchesShortcut(event, 'mod+end') || matchesShortcut(event, 'mod+arrowdown');

    if (event.defaultPrevented || !jump || document.querySelector('[role="dialog"][aria-modal="true"]')) {
      return;
    }

    event.preventDefault();
    list?.scrollToEnd();
  }

  const showThinking = $derived(prefs.value?.showThinking ?? true);

  /**
   * While the agent works with nothing visible to show for it (before the first output, while hidden reasoning
   * streams, between steps), the last row says so; streaming text and running tools show their own progress.
   */
  const working = $derived.by((): string | null => {
    if (!session.streaming) {
      return null;
    }

    const last = session.chat.items.at(-1);
    if (Object.values(session.chat.toolCalls).some((call) => call.status === 'running')) {
      return null;
    }
    if (last?.kind === 'message' && last.role === 'assistant' && last.streaming) {
      if (last.blocks.some((block) => block.type === 'text' || block.type === 'tool' || (block.type === 'thinking' && showThinking))) {
        return null;
      }

      return t('chat.working.thinking');
    }

    return t('chat.working.working');
  });

  async function loadEarlier(): Promise<void> {
    loadingEarlier = true;

    try {
      await list?.keepPosition(() => session.loadHistory());
    } finally {
      loadingEarlier = false;
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- The conversation scrolls inside an equal gap at the top and the bottom, so text never runs under the bar above or the composer below. -->
<div class="chat-stage relative min-h-0 flex-1 py-3">
  <!-- Both edges fade the same short distance instead of ending in a hard cut: gradient overlays, not a mask, since a mask
     makes WebKitGTK composite the conversation as an image and its text loses its sharpness. -->
  <VirtualList
    bind:this={list}
    bind:atBottom
    items={session.chat.items}
    variable
    stick
    estimate={ESTIMATED_ROW_PX}
    label={t('chat.list.label')}
    key={(item) => item.key}
    class="size-full [scrollbar-gutter:stable_both-edges]"
  >
    {#snippet row(item, index)}
      <div class={cx('chat-col py-2', index === 0 && 'pt-4', index === session.chat.items.length - 1 && 'pb-4')} in:rise|global={{ y: 8, duration: entrance(item) }}>
        {#if index === 0 && session.hasEarlier}
          <div class="mb-3 flex justify-center">
            <Button variant="ghost" loading={loadingEarlier} onclick={loadEarlier}>
              {#snippet leading()}<History size={16} aria-hidden="true" />{/snippet}
              {t('chat.list.loadEarlier')}
            </Button>
          </div>
        {/if}
        <MessageRow {item} chat={session.chat} sentImages={session.sentImages} />
        {#if working && index === session.chat.items.length - 1}
          <WorkingIndicator label={working} />
        {/if}
      </div>
    {/snippet}
  </VirtualList>

  <div class="pointer-events-none absolute inset-x-0 top-3 h-4 bg-gradient-to-b from-canvas to-transparent" aria-hidden="true"></div>
  <div class="pointer-events-none absolute inset-x-0 bottom-3 h-4 bg-gradient-to-t from-canvas to-transparent" aria-hidden="true"></div>

  {#if session.chat.items.length === 0 && session.noModels && !session.ended}
    <!-- A harness with no model to use cannot answer anything, so the page says that instead of inviting a question. -->
    <div class="absolute inset-0 grid place-items-center">
      <EmptyState icon={KeyRound} title={t('chat.noModels.title', { harness: harness.get(session.info.harness)?.info.name ?? '' })} body={t('chat.noModels.body', { harness: harness.get(session.info.harness)?.info.name ?? '' })}>
        {#snippet action()}
          <Button variant="primary" disabled={rechecking} onclick={() => void recheckModels()}>
            {#snippet leading()}<RefreshCw size={16} aria-hidden="true" class={rechecking ? 'animate-spin' : undefined} />{/snippet}
            {t('chat.noModels.action')}
          </Button>
        {/snippet}
      </EmptyState>
    </div>
  {:else if session.chat.items.length === 0}
    <p class="pointer-events-none absolute inset-0 grid place-items-center px-8 text-center text-base text-fg-3">
      {session.ended ? t('chat.list.ended') : t('chat.list.empty')}
    </p>
  {/if}

  {#if !atBottom}
    <div class="pointer-events-none absolute inset-x-0 bottom-3 flex justify-center">
      <Button class="pointer-events-auto shadow-float" aria-keyshortcuts="Control+End Meta+End Control+ArrowDown Meta+ArrowDown" onclick={() => list?.scrollToEnd()}>
        {#snippet leading()}<ArrowDown size={16} aria-hidden="true" />{/snippet}
        {#snippet trailing()}<span class="flex items-center" aria-hidden="true"><Kbd keys="mod+arrowdown" small /></span>{/snippet}
        {t('chat.list.jump')}
      </Button>
    </div>
  {/if}
</div>
