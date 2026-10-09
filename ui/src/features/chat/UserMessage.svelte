<script lang="ts">
  import type { MessageItem } from '$lib/chat/types';

  interface Props {
    item: MessageItem;
  }

  let { item }: Props = $props();

  const text = $derived(item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n'));
</script>

<div class="flex justify-end">
  <article aria-label="Your message" class="max-w-[85%] rounded-lg rounded-br-sm border border-line-strong bg-accent-wash px-4 py-2.5 text-base break-words whitespace-pre-wrap text-fg">
    {text}
  </article>
</div>
