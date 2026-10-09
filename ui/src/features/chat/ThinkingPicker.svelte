<script lang="ts">
  import Brain from '@lucide/svelte/icons/brain';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';

  import MenuItem from './MenuItem.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const level = $derived(session.chat.session?.thinkingLevel ?? null);
  const supported = $derived(session.chat.session?.model?.reasoning !== false);

  $effect(() => {
    if (session.ui.thinkingPicker) {
      void session.loadThinkingLevels();
    }
  });
</script>

{#if supported}
  <Popover label="Thinking level" kind="menu" bind:open={session.ui.thinkingPicker} class="min-w-44">
    {#snippet trigger({ props })}
      <button type="button" {...props} class="inline-flex h-8 items-center gap-2 rounded-md border border-line bg-surface-2 px-2.5 text-sm text-fg-2 transition-colors hover:bg-surface-3 hover:text-fg">
        <Brain size={16} aria-hidden="true" />
        <span>Thinking: {level ?? 'default'}</span>
        <ChevronDown size={16} aria-hidden="true" class="text-fg-3" />
      </button>
    {/snippet}

    {#snippet children({ close })}
      {#if session.thinkingLevels === null}
        <p class="px-2.5 py-2 text-sm text-fg-3" role="status">Loading…</p>
      {:else}
        {#each session.thinkingLevels as option (option)}
          <MenuItem
            label={option}
            selected={option === level}
            onselect={() => {
              void session.chooseThinking(option);
              close();
            }}
          />
        {/each}
      {/if}
    {/snippet}
  </Popover>
{/if}
