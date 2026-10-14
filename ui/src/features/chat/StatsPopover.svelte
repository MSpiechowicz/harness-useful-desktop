<script lang="ts">
  import Activity from '@lucide/svelte/icons/activity';

  import Popover from '$lib/components/Popover.svelte';
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

<Popover label={t('chat.stats.popover')} kind="dialog" align="end" bind:open={session.ui.stats} class="w-80">
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      aria-label={t('chat.stats.popover')}
      class="inline-flex h-8 items-center gap-1.5 rounded-md px-2 text-sm text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg"
    >
      <Activity size={16} aria-hidden="true" class="shrink-0" />
      {t('chat.stats.trigger')}
    </button>
  {/snippet}

  {#snippet children()}
    <div class="flex flex-col gap-2 p-1.5">
      <h3 class="text-sm font-semibold text-fg">{t('chat.stats.heading')}</h3>

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
    </div>
  {/snippet}
</Popover>
