<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import { tick, untrack } from 'svelte';

  import { provideToolContext } from '$lib/chat/tool-context';
  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import type { TranscriptItem } from '$lib/chat/types';
  import type { SessionDetail } from '$lib/stores/sessions.svelte';
  import { cx } from '$lib/utils/cx';

  import MessageRow from '../chat/MessageRow.svelte';
  import { focusRow, rowMessage, transcriptRow } from './transcript-focus';

  interface Props {
    detail: SessionDetail;
    project: string;
    /** A tree node to bring into view; a new `nonce` repeats the jump for the same node. */
    focus?: { id: string; nonce: number } | null;
    /** The node picked for forking, outlined when it is one of the user's messages. */
    picked?: string | null;
    /** Picks one of the user's messages as the fork point (the same as picking it in the tree). */
    onpick?: (id: string) => void;
  }

  let { detail, project, focus = null, picked = null, onpick }: Props = $props();

  /** The user's own messages can be picked as a fork point; everything else is read-only. */
  function forkable(index: number): string | null {
    const message = rowMessage(detail.messages, index);

    return message?.role === 'user' && message.id ? message.id : null;
  }

  const ESTIMATED_ROW_PX = 96;
  const HIGHLIGHT_MS = 1600;
  /** Pages read at most while looking for a node further down the transcript. */
  const MAX_PAGES = 40;

  let list: VirtualList<TranscriptItem> | undefined = $state();
  let highlighted = $state<string | null>(null);
  let highlightTimer: ReturnType<typeof setTimeout> | undefined;

  /** Scrolls to the message of tree node `id` (reading later pages first if it is not loaded yet) and flashes it. */
  async function reveal(id: string): Promise<void> {
    const onPath = detail.tree?.nodes.find((node) => node.id === id)?.onActivePath ?? false;

    for (let page = 0; page < MAX_PAGES && onPath && transcriptRow(detail.messages, id) === null && detail.nextCursor !== null; page += 1) {
      await detail.loadMore();
    }

    // Entries before the first message (the session header, an opening model change) have no message above them.
    const row = focusRow(detail.tree?.nodes ?? [], detail.messages, id) ?? (chat.items.length > 0 ? 0 : null);
    if (row === null) {
      return;
    }

    await tick();
    list?.scrollToIndex(row);
    // Rows above may change height once rendered; a second pass lands on the measured position.
    requestAnimationFrame(() => list?.scrollToIndex(row));

    highlighted = chat.items[row]?.key ?? null;
    clearTimeout(highlightTimer);
    highlightTimer = setTimeout(() => (highlighted = null), HIGHLIGHT_MS);
  }

  $effect(() => {
    if (focus) {
      const { id } = focus;
      untrack(() => void reveal(id));
    }
  });

  $effect(() => () => clearTimeout(highlightTimer));

  provideToolContext({ project: untrack(() => project), label: 'this session' });

  const chat = $derived(detail.transcript);
</script>

<div class="flex size-full min-h-0 flex-col">
  {#if detail.transcriptStatus === 'loading'}
    <div class="flex flex-col gap-3 p-6" role="status" aria-label="Loading transcript">
      <Skeleton width="60%" height="2.5rem" />
      <Skeleton height="4rem" />
      <Skeleton width="80%" height="2.5rem" />
    </div>
  {:else if detail.transcriptStatus === 'error'}
    <div class="flex flex-col items-center gap-3 p-8 text-center" role="alert">
      <CircleAlert size={24} class="text-danger" aria-hidden="true" />
      <p class="text-base text-fg-2">{detail.transcriptError?.message ?? 'The transcript could not be read.'}</p>
      <Button onclick={() => detail.reload()}>Try again</Button>
    </div>
  {:else if chat.items.length === 0}
    <p class="grid size-full place-items-center p-8 text-center text-base text-fg-3" role="status">This session has no messages yet.</p>
  {:else}
    <VirtualList bind:this={list} items={chat.items} variable estimate={ESTIMATED_ROW_PX} overscan={10} label="Session transcript" key={(item) => item.key} class="min-h-0 flex-1">
      {#snippet row(item, index)}
        {@const forkId = onpick ? forkable(index) : null}
        <div class={cx('w-full max-w-3xl px-6 py-2', index === 0 && 'pt-6', index === chat.items.length - 1 && 'pb-8')}>
          <!-- The negative margin and matching padding let the highlight frame the message without moving it. -->
          {#if forkId}
            <div
              role="button"
              tabindex="0"
              aria-pressed={picked === forkId}
              aria-label="Pick this message to fork from"
              title="Pick this message to fork from"
              class={cx(
                '-m-2 cursor-pointer rounded-lg border p-2 transition-colors duration-[var(--dur-fast)]',
                picked === forkId ? 'border-accent/40 bg-accent-wash' : highlighted === item.key ? 'border-transparent bg-accent-wash' : 'border-transparent hover:bg-hover',
              )}
              onclick={() => onpick?.(forkId)}
              onkeydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onpick?.(forkId);
                }
              }}
            >
              <MessageRow {item} {chat} />
            </div>
          {:else}
            <div class={cx('-m-2 rounded-lg p-2 transition-colors duration-[var(--dur-slow)]', highlighted === item.key ? 'bg-accent-wash' : 'bg-transparent')}>
              <MessageRow {item} {chat} />
            </div>
          {/if}

          {#if index === chat.items.length - 1 && detail.nextCursor !== null}
            <div class="mt-3 flex items-center justify-center gap-3">
              <span class="tabular text-xs text-fg-3">{detail.messages.length} of {detail.total} messages</span>
              <Button size="sm" loading={detail.loadingMore} onclick={() => void detail.loadMore()}>Load more messages</Button>
            </div>
          {/if}
        </div>
      {/snippet}
    </VirtualList>
  {/if}
</div>
