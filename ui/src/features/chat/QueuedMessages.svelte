<script lang="ts">
  import X from '@lucide/svelte/icons/x';

  import Badge from '$lib/components/Badge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import { TAKE_BACK, type QueuedRow } from '$lib/chat/composer';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    rows: QueuedRow[];
    /** The harness can withdraw a waiting message (OMP); without it the list only shows what waits. */
    removable: boolean;
    onremove: (row: QueuedRow) => void;
  }

  let { rows, removable, onremove }: Props = $props();
</script>

<!-- What waits for the agent, above the composer: one row per message, each with its own remove button. -->
<section class="chat-radius mb-2 overflow-hidden border-card border-line bg-surface-1 bg-clip-padding" aria-label={t('chat.composer.queued')}>
  <header class="flex items-center justify-between gap-3 px-3 pt-2 pb-1 text-xs text-fg-3">
    <span class="font-medium tracking-wide uppercase">{t('chat.composer.queued')} · <span class="tabular">{rows.length}</span></span>
    {#if removable}
      <span class="flex items-center gap-1.5">{t('chat.composer.queue.takeBack')}<Kbd keys={TAKE_BACK} small /></span>
    {/if}
  </header>

  <ul class="flex max-h-40 flex-col overflow-y-auto pb-1">
    {#each rows as row, index (`${row.queue}${index}`)}
      <li class="flex min-h-9 items-center gap-2.5 py-1 pr-1.5 pl-3">
        {#if !row.withdrawable}
          <Badge tone="neutral" class="w-14 justify-center">{t('chat.composer.queue.sent')}</Badge>
        {:else if row.queue === 'steer'}
          <Badge tone="accent" class="w-14 justify-center">{t('chat.composer.steer')}</Badge>
        {:else}
          <Badge tone="info" class="w-14 justify-center">{t('chat.composer.queue.next')}</Badge>
        {/if}
        <span class="min-w-0 flex-1 truncate text-sm text-fg-2" title={row.message}>{row.message}</span>
        {#if removable && row.withdrawable}
          <IconButton icon={X} label={t('chat.composer.queue.remove')} class="size-7! shrink-0" iconSize={14} onclick={() => onremove(row)} />
        {/if}
      </li>
    {/each}
  </ul>
</section>
