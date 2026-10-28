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

  /** The note titles from the header lines (`#12 [kind, scope, date, from …] Title {entities}`), then the script names, for the summary. */
  const titles = $derived(
    [
      ...recall.notes.split('\n').flatMap((line) => {
        const title = /^#\d+ \[.*?\] (.*?)(?: \{.*\})?$/.exec(line)?.[1];
        return title ? [title] : [];
      }),
      ...recall.scripts.split('\n').flatMap((line) => {
        const name = /^ {2}- ([a-z0-9][a-z0-9_-]*)/.exec(line)?.[1];
        return name ? [name] : [];
      }),
    ].join(' · '),
  );

  /** "2 notes", "1 script" or "2 notes · 1 script": a part with nothing in it is left out. */
  const counts = $derived(
    [recall.count > 0 ? tn('chat.user.recallCount', recall.count) : '', recall.scriptCount > 0 ? tn('chat.user.recallScriptCount', recall.scriptCount) : '']
      .filter(Boolean)
      .join(' · '),
  );
</script>

<!-- The notes and saved scripts the app added to this message, in the same row shape as the agent's tool calls. -->
<ChatRow {open} ontoggle={() => (open = !open)} tone="info">
  {#snippet icon()}<BookOpen size={16} aria-hidden="true" />{/snippet}

  {#snippet title()}
    <span class="font-medium text-fg">{t('chat.user.recallTitle')}</span>
    {#if titles}<span class="ml-2 font-mono text-xs text-fg-3">{titles}</span>{/if}
  {/snippet}

  {#snippet status()}
    <StatusPill tone="info" label={counts} wide={recall.count > 0 && recall.scriptCount > 0} />
  {/snippet}

  {#snippet body()}
    <div class="flex flex-col gap-2 p-4">
      {#if recall.count > 0}
        <p class="text-xs text-fg-3">{t('chat.user.recallCaption')}</p>
        <!-- Same panel as a tool's output (TerminalBlock), as a labelled region. -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
        <pre role="region" tabindex="0" aria-label={t('chat.user.recallNotes')} class="chat-radius max-h-72 overflow-auto border border-line bg-canvas px-3.5 py-3 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-fg-2">{recall.notes}</pre>
      {/if}
      {#if recall.scriptCount > 0}
        <p class="text-xs text-fg-3">{t('chat.user.recallScriptsCaption')}</p>
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
        <pre role="region" tabindex="0" aria-label={t('chat.user.recallScripts')} class="chat-radius max-h-72 overflow-auto border border-line bg-canvas px-3.5 py-3 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-fg-2">{recall.scripts}</pre>
      {/if}
    </div>
  {/snippet}
</ChatRow>
