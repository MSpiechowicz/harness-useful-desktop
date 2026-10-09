<script lang="ts">
  import type { MessageItem } from '$lib/chat/types';

  interface Props {
    item: MessageItem;
  }

  let { item }: Props = $props();

  const LABELS: Record<string, string> = {
    bash: 'Command',
    custom: 'Extension',
    system: 'System',
    branchSummary: 'Branch summary',
    compactionSummary: 'Compacted context',
    other: 'Message',
  };

  const text = $derived(item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n'));
</script>

<article class="rounded-md border border-line bg-surface-1/50 px-3.5 py-2.5 text-sm text-fg-2">
  <p class="mb-1 text-xs font-medium tracking-wide text-fg-3 uppercase">{LABELS[item.role] ?? 'Message'}</p>
  <p class="break-words whitespace-pre-wrap">{text}</p>
</article>
