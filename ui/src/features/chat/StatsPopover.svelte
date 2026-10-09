<script lang="ts">
  import ChartNoAxesColumn from '@lucide/svelte/icons/chart-no-axes-column';

  import IconButton from '$lib/components/IconButton.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { formatCount } from '$lib/utils/format';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const stats = $derived(session.stats);
  const rows = $derived(
    stats
      ? [
          ['Messages', `${stats.userMessages} sent · ${stats.assistantMessages} replies`],
          ['Tool calls', formatCount(stats.toolCalls)],
          ['Input tokens', formatCount(stats.tokens.input)],
          ['Output tokens', formatCount(stats.tokens.output)],
          ['Cache read / write', `${formatCount(stats.tokens.cacheRead)} / ${formatCount(stats.tokens.cacheWrite)}`],
          ['Total tokens', formatCount(stats.tokens.total)],
          ['Cost', stats.costUsd === null ? 'not reported' : `$${stats.costUsd.toFixed(4)}`],
        ]
      : [],
  );

  $effect(() => {
    if (session.ui.stats) {
      void session.loadStats();
    }
  });
</script>

<Popover label="Session statistics" kind="dialog" align="end" bind:open={session.ui.stats} class="w-80">
  {#snippet trigger({ props })}
    <IconButton icon={ChartNoAxesColumn} label="Session statistics" size="sm" side="bottom" {...props} />
  {/snippet}

  {#snippet children()}
    <div class="flex flex-col gap-2 p-1.5">
      <h3 class="text-sm font-semibold text-fg">This session</h3>

      {#if stats}
        <dl class="tabular grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
          {#each rows as [name, value] (name)}
            <dt class="text-fg-3">{name}</dt>
            <dd class="text-right text-fg">{value}</dd>
          {/each}
        </dl>
      {:else}
        <p class="text-sm text-fg-3" role="status">Loading statistics…</p>
      {/if}
    </div>
  {/snippet}
</Popover>
