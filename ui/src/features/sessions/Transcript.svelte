<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import { untrack } from 'svelte';

  import { provideToolContext } from '$lib/chat/tool-context';
  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import type { SessionDetail } from '$lib/stores/sessions.svelte';

  import MessageRow from '../chat/MessageRow.svelte';

  interface Props {
    detail: SessionDetail;
    project: string;
  }

  let { detail, project }: Props = $props();

  const ESTIMATED_ROW_PX = 96;

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
    <VirtualList items={chat.items} variable estimate={ESTIMATED_ROW_PX} overscan={10} label="Session transcript" key={(item) => item.key} class="min-h-0 flex-1">
      {#snippet row(item, index)}
        <div class="mx-auto w-full max-w-3xl px-6 py-2">
          <MessageRow {item} {chat} />

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
