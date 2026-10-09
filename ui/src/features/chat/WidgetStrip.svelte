<script lang="ts">
  import type { ChatState } from '$lib/chat/types';

  interface Props {
    chat: ChatState;
    placement: 'above' | 'below';
  }

  let { chat, placement }: Props = $props();

  const widgets = $derived(Object.entries(chat.widgets).filter(([, widget]) => (widget.placement === 'belowEditor') === (placement === 'below')));
  const statuses = $derived(placement === 'above' ? Object.entries(chat.statuses) : []);
</script>

{#if widgets.length > 0 || statuses.length > 0}
  <div class="chat-col flex flex-wrap items-start gap-2 pb-2">
    {#each statuses as [name, text] (name)}
      <p class="rounded-full bg-surface-3 px-3 py-1 text-xs text-fg-2" role="status">{text}</p>
    {/each}
    {#each widgets as [name, widget] (name)}
      <div class="rounded-md border border-line bg-surface-1/60 px-3 py-2 text-sm text-fg-2">
        {#each widget.lines as line, index (index)}<p class="break-words">{line}</p>{/each}
      </div>
    {/each}
  </div>
{/if}
