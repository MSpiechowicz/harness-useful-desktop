<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import History from '@lucide/svelte/icons/history';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { TranscriptItem } from '$lib/chat/types';
  import { rise } from '$lib/design/motion';

  import MessageRow from './MessageRow.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const ESTIMATED_ROW_PX = 96;
  const ENTRANCE_MS = 240;

  let list: VirtualList<TranscriptItem> | undefined = $state();
  let atBottom = $state(true);
  let loadingEarlier = $state(false);

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

  async function loadEarlier(): Promise<void> {
    loadingEarlier = true;

    try {
      await list?.keepPosition(() => session.loadHistory());
    } finally {
      loadingEarlier = false;
    }
  }
</script>

<div class="relative min-h-0 flex-1">
  <VirtualList bind:this={list} bind:atBottom items={session.chat.items} variable stick estimate={ESTIMATED_ROW_PX} label="Conversation" key={(item) => item.key} class="size-full">
    {#snippet row(item, index)}
      <div class="mx-auto w-full max-w-3xl px-6 py-2" in:rise|global={{ y: 8, duration: entrance(item) }}>
        {#if index === 0 && session.hasEarlier}
          <div class="mb-3 flex justify-center">
            <Button size="sm" variant="ghost" loading={loadingEarlier} onclick={loadEarlier}>
              {#snippet leading()}<History size={14} aria-hidden="true" />{/snippet}
              Load earlier messages
            </Button>
          </div>
        {/if}
        <MessageRow {item} chat={session.chat} />
      </div>
    {/snippet}
  </VirtualList>

  {#if session.chat.items.length === 0}
    <p class="pointer-events-none absolute inset-0 grid place-items-center px-8 text-center text-base text-fg-3">
      {session.ended ? 'This session has ended.' : 'Ask anything. Type / for commands or ! to run a shell command.'}
    </p>
  {/if}

  {#if !atBottom}
    <div class="pointer-events-none absolute inset-x-0 bottom-3 flex justify-center">
      <Button size="sm" class="pointer-events-auto shadow-float" onclick={() => list?.scrollToEnd()}>
        {#snippet leading()}<ArrowDown size={14} aria-hidden="true" />{/snippet}
        Jump to latest
      </Button>
    </div>
  {/if}
</div>
