<script lang="ts">
  import Dialog from '$lib/components/Dialog.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { formatCost, formatCount } from '$lib/utils/format';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const stats = $derived(session.stats);
  const rows = $derived(
    stats
      ? [
          [
            t('chat.stats.messages'),
            t('chat.stats.messagesValue', { sent: t('chat.stats.sent', { count: stats.userMessages }), replies: tn('chat.stats.replies', stats.assistantMessages) }),
          ],
          [t('chat.stats.toolCalls'), formatCount(stats.toolCalls)],
          [t('chat.stats.input'), formatCount(stats.tokens.input)],
          [t('chat.stats.output'), formatCount(stats.tokens.output)],
          [t('chat.stats.cache'), `${formatCount(stats.tokens.cacheRead)} / ${formatCount(stats.tokens.cacheWrite)}`],
          [t('chat.stats.total'), formatCount(stats.tokens.total)],
          [t('chat.stats.cost'), stats.costUsd === null ? t('chat.stats.notReported') : formatCost(stats.costUsd)],
        ]
      : [],
  );

  $effect(() => {
    if (session.ui.stats) {
      void session.loadStats();
    }
  });
</script>

<!-- Opened from the top bar's menu, and by the /stats command. -->
<Dialog bind:open={session.ui.stats} title={t('chat.stats.heading')} size="sm">
  {#if stats}
    <dl class="tabular grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
      {#each rows as [name, value] (name)}
        <dt class="text-fg-3">{name}</dt>
        <dd class="text-right text-fg">{value}</dd>
      {/each}
    </dl>
  {:else}
    <p class="text-sm text-fg-3" role="status">{t('chat.stats.loading')}</p>
  {/if}
</Dialog>
