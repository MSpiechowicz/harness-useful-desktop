<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Cpu from '@lucide/svelte/icons/cpu';

  import Popover from '$lib/components/Popover.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { rememberModel } from '$lib/chat/chat-defaults';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { ModelOption } from '$lib/ipc/bindings/ModelOption';
  import { fuzzyMatch } from '$lib/utils/fuzzy';

  import { groupByProvider } from './models';
  import MenuItem from './MenuItem.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let query = $state('');

  const current = $derived(session.chat.session?.model ?? null);
  const matches = $derived((session.models ?? []).filter((model) => fuzzyMatch(query, `${model.provider}/${model.id} ${model.name ?? ''}`) !== null));
  const groups = $derived(groupByProvider(matches));

  $effect(() => {
    if (session.ui.modelPicker) {
      void session.loadModels();
    }
  });

  function sameModel(model: ModelOption): boolean {
    return current?.provider === model.provider && current.id === model.id;
  }

  /** Home/End belong to the text field here, not to the list navigation of the popover. */
  function keepCaretKeys(event: KeyboardEvent): void {
    if (event.key === 'Home' || event.key === 'End') {
      event.stopPropagation();
    }
  }
</script>

<Popover label="Choose model" kind="dialog" side="top" align="end" bind:open={session.ui.modelPicker} class="w-80">
  {#snippet trigger({ props })}
    <button type="button" {...props} class="inline-flex h-8 items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg max-w-64">
      <Cpu size={16} aria-hidden="true" class="shrink-0" />
      <span class="truncate">{current ? (current.name ?? current.id) : 'Choose model'}</span>
      <ChevronDown size={16} aria-hidden="true" class="shrink-0 text-fg-3" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="flex flex-col gap-2" role="presentation" onkeydown={keepCaretKeys}>
      <SearchField bind:value={query} label="Filter models" placeholder="Filter models…" />

      <div class="flex max-h-72 flex-col gap-2 overflow-y-auto" role="group" aria-label="Models">
        {#if session.models === null}
          <p class="px-2 py-3 text-sm text-fg-3" role="status">Loading models…</p>
        {:else if groups.length === 0}
          <p class="px-2 py-3 text-sm text-fg-3" role="status">No models match.</p>
        {/if}

        {#each groups as group (group.provider)}
          <div class="flex flex-col">
            <p class="px-2.5 py-1 text-xs font-medium tracking-wide text-fg-3 uppercase">{group.provider}</p>
            {#each group.models as model (model.id)}
              <MenuItem
                label={model.name ?? model.id}
                detail={model.contextWindow ? `${Math.round(model.contextWindow / 1000)}k` : null}
                selected={sameModel(model)}
                onselect={() => {
                  void session.chooseModel(model);
                  rememberModel(session.info.harness, model);
                  close();
                }}
              />
            {/each}
          </div>
        {/each}
      </div>
    </div>
  {/snippet}
</Popover>
