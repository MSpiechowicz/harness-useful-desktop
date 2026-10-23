<script lang="ts">
  import Gauge from '@lucide/svelte/icons/gauge';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Popover from '$lib/components/Popover.svelte';
  import { rememberThinking } from '$lib/chat/chat-defaults';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import MenuItem from './MenuItem.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const level = $derived(session.chat.session?.thinkingLevel ?? null);
  const model = $derived(session.chat.session?.model ?? null);
  // Levels belong to a model: none is offered before the session reports one.
  const supported = $derived(model !== null && model.reasoning !== false);
  const modelKey = $derived(model === null ? null : `${model.provider}/${model.id}`);
  // An unknown level is shown only when the model offers levels to pick from: otherwise the
  // picker would only say "default" (hide unknown indicators).
  const shown = $derived(supported && (level !== null || (session.thinkingLevels?.length ?? 0) > 0));

  let levelsOf: string | null = null;

  $effect(() => {
    if (!supported || modelKey === null) {
      return;
    }

    // Each model has its own levels: ask again after a model change.
    if (levelsOf !== modelKey) {
      levelsOf = modelKey;
      session.thinkingLevels = null;
    }

    void session.loadThinkingLevels();
  });
</script>

{#if shown}
  <Popover label={t('chat.thinking.picker')} kind="menu" side="top" align="end" bind:open={session.ui.thinkingPicker} class="min-w-44">
    {#snippet trigger({ props })}
      <button type="button" {...props} class="inline-flex h-control items-center gap-1.5 rounded-md pr-1 pl-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg">
        <Gauge size={16} aria-hidden="true" />
        <!-- The icon says "thinking"; the word stays in the accessible name. -->
        <span><span class="sr-only">{t('chat.thinking.prefix')}</span>{level ?? t('chat.thinking.default')}</span>
        <ChevronDown size={16} aria-hidden="true" class="text-fg-3" />
      </button>
    {/snippet}

    {#snippet children({ close })}
      {#if session.thinkingLevels === null}
        <p class="px-2.5 py-2 text-sm text-fg-3" role="status">{t('chat.thinking.loading')}</p>
      {:else}
        {#each session.thinkingLevels as option (option)}
          <MenuItem
            label={option}
            selected={option === level}
            onselect={() => {
              void session.chooseThinking(option);
              rememberThinking(session.info.harness, option);
              close();
            }}
          />
        {/each}
      {/if}
    {/snippet}
  </Popover>
{/if}
