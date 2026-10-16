<script lang="ts">
  import BookOpen from '@lucide/svelte/icons/book-open';

  import type { MemoryRecall } from '$lib/chat/memory-recall';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import ChatRow from './ChatRow.svelte';
  import StatusPill from './StatusPill.svelte';

  interface Props {
    recall: MemoryRecall;
  }

  let { recall }: Props = $props();

  let open = $state(false);

  /** The note titles from the header lines (`#12 [kind, scope, date, from …] Title {entities}`), for the summary. */
  const titles = $derived(
    recall.notes
      .split('\n')
      .flatMap((line) => {
        const title = /^#\d+ \[.*?\] (.*?)(?: \{.*\})?$/.exec(line)?.[1];
        return title ? [title] : [];
      })
      .join(' · '),
  );
</script>

<!-- The notes the app added to this message, in the same row shape as the agent's tool calls. -->
<ChatRow {open} ontoggle={() => (open = !open)}>
  {#snippet icon()}<BookOpen size={16} aria-hidden="true" />{/snippet}

  {#snippet title()}
    <span class="font-medium text-fg">{t('chat.user.recallTitle')}</span>
    {#if titles}<span class="ml-2 font-mono text-xs text-fg-3">{titles}</span>{/if}
  {/snippet}

  {#snippet status()}
    <StatusPill tone="info" label={tn('chat.user.recallCount', recall.count)}>
      {#snippet icon()}<BookOpen size={16} aria-hidden="true" />{/snippet}
    </StatusPill>
  {/snippet}

  {#snippet body()}
    <div class="flex flex-col gap-2 p-4">
      <p class="text-xs text-fg-3">{t('chat.user.recallCaption')}</p>
      <!-- Same panel as a tool's output (TerminalBlock), as a labelled region. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
      <pre role="region" tabindex="0" aria-label={t('chat.user.recallNotes')} class="chat-radius max-h-72 overflow-auto border border-line bg-canvas px-3.5 py-3 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-fg-2">{recall.notes}</pre>
    </div>
  {/snippet}
</ChatRow>
