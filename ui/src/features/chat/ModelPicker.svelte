<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Cpu from '@lucide/svelte/icons/cpu';

  import Popover from '$lib/components/Popover.svelte';
  import { rememberModel } from '$lib/chat/chat-defaults';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ModelChoices from './ModelChoices.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const current = $derived(session.chat.session?.model ?? null);

  $effect(() => {
    if (session.ui.modelPicker) {
      void session.loadModels();
    }
  });
</script>

<Popover label={t('chat.model.popover')} kind="dialog" side="top" align="end" bind:open={session.ui.modelPicker} class="w-80">
  {#snippet trigger({ props })}
    <button type="button" {...props} class="inline-flex h-control items-center gap-1.5 rounded-md pr-1 pl-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg max-w-64">
      <Cpu size={16} aria-hidden="true" class="shrink-0" />
      <span class="truncate">{current ? (current.name ?? current.id) : t('chat.model.popover')}</span>
      <ChevronDown size={16} aria-hidden="true" class="shrink-0 text-fg-3" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <ModelChoices
      models={session.models}
      {current}
      onchoose={(model) => {
        void session.chooseModel(model);
        rememberModel(session.info.harness, model);
        close();
      }}
    />
  {/snippet}
</Popover>
