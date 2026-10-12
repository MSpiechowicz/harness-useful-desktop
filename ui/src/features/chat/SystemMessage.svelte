<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageItem } from '$lib/chat/types';

  interface Props {
    item: MessageItem;
  }

  let { item }: Props = $props();

  function label(role: string): string {
    switch (role) {
      case 'bash':
        return t('chat.system.bash');

      case 'custom':
        return t('chat.system.custom');

      case 'system':
        return t('chat.system.system');

      case 'branchSummary':
        return t('chat.system.branchSummary');

      case 'compactionSummary':
        return t('chat.system.compactionSummary');

      default:
        return t('chat.system.message');
    }
  }

  const text = $derived(item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n'));
</script>

<article class="rounded-md border border-line bg-surface-1/50 px-3.5 py-2.5 text-sm text-fg-2">
  <p class="mb-1 text-xs font-medium tracking-wide text-fg-3 uppercase">{label(item.role)}</p>
  <p class="break-words whitespace-pre-wrap">{text}</p>
</article>
