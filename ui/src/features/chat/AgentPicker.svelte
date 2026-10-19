<script lang="ts">
  import Bot from '@lucide/svelte/icons/bot';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';

  import MenuItem from './MenuItem.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const current = $derived(session.chat.session?.agent ?? null);
  const name = $derived(session.agents?.find((option) => option.id === current)?.name ?? current);
  // Only a harness with an agent picker, and a chat that has reported its agent, has one to show.
  const supported = $derived(Boolean(harness.get(session.info.harness)?.capabilities.agentPicker) && current !== null);
  /** The harness reads the agent once per turn, so it refuses a switch while one runs. */
  const locked = $derived(session.streaming);

  // The list is short and the trigger names the agent, so it is asked for as soon as the picker shows.
  $effect(() => {
    if (supported) {
      void session.loadAgents();
    }
  });

  $effect(() => {
    if (locked) {
      session.ui.agentPicker = false;
    }
  });
</script>

{#if supported}
  <Popover label={t('chat.agent.picker')} kind="menu" side="top" align="end" bind:open={session.ui.agentPicker} class="min-w-52">
    {#snippet trigger({ props })}
      <button
        type="button"
        {...props}
        title={locked ? t('chat.agent.locked') : undefined}
        disabled={locked}
        class="inline-flex h-control items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-fg-3 aria-expanded:bg-hover aria-expanded:text-fg"
      >
        <Bot size={16} aria-hidden="true" />
        <!-- The icon says "agent"; the word stays in the accessible name. -->
        <span><span class="sr-only">{t('chat.agent.prefix')}</span>{name}</span>
        <ChevronDown size={16} aria-hidden="true" class="text-fg-3" />
      </button>
    {/snippet}

    {#snippet children({ close })}
      {#if session.agents === null}
        <p class="px-2.5 py-2 text-sm text-fg-3" role="status">{t('chat.agent.loading')}</p>
      {:else}
        {#each session.agents as option (option.id)}
          <MenuItem
            label={option.name}
            hint={option.description}
            selected={option.id === current}
            onselect={() => {
              void session.chooseAgent(option);
              close();
            }}
          />
        {/each}
      {/if}
    {/snippet}
  </Popover>
{/if}
