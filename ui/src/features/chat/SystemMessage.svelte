<script lang="ts">
  import Markdown from '$lib/components/Markdown.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageItem } from '$lib/chat/types';

  import { unwrapNotice } from './system-notice';

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

  const raw = $derived(item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n'));
  // A harness notice arrives wrapped in a tag of its own: its text is shown, under the tag's name.
  const notice = $derived(item.role === 'bash' ? { title: null, body: raw, details: null } : unwrapNotice(raw));
</script>

<article class="chat-radius border border-line bg-surface-1/50 px-3.5 py-2.5 text-sm text-fg-2">
  <p class="mb-1 text-xs font-medium tracking-wide text-fg-3 uppercase">{notice.title ?? label(item.role)}</p>
  {#if item.role === 'bash'}
    <p class="break-words whitespace-pre-wrap">{notice.body}</p>
  {:else}
    <Markdown source={notice.body} />
  {/if}
  {#if notice.details}
    <!-- Command output and errors keep their lines and scroll inside the card instead of filling the chat. -->
    <pre class="mt-2 max-h-60 overflow-auto rounded-md border border-line bg-surface-2 p-2.5 font-mono text-xs break-words whitespace-pre-wrap text-fg-2">{notice.details}</pre>
  {/if}
</article>
