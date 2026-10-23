<script lang="ts">
  import type { Snippet } from 'svelte';

  import SearchField from '$lib/components/SearchField.svelte';
  import type { ModelOption } from '$lib/ipc/bindings/ModelOption';
  import { fuzzyMatch } from '$lib/utils/fuzzy';
  import { t } from '$lib/i18n/i18n.svelte';

  import { groupByProvider } from './models';
  import MenuItem from './MenuItem.svelte';

  interface Props {
    /** The models to choose from; null while they are being asked for. */
    models: ModelOption[] | null;
    /** The model in use: its row is checked. */
    current: { provider: string; id: string } | null;
    /** The models could not be asked for: said instead of "loading". */
    unavailable?: string | null;
    /** Rows above the groups (a choice that is no model, like "the harness default"). */
    leading?: Snippet;
    onchoose: (model: ModelOption) => void;
  }

  let { models, current, unavailable = null, leading, onchoose }: Props = $props();

  let query = $state('');

  const matches = $derived((models ?? []).filter((model) => fuzzyMatch(query, `${model.provider}/${model.id} ${model.name ?? ''}`) !== null));
  const groups = $derived(groupByProvider(matches));

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

<div class="flex flex-col gap-2" role="presentation" onkeydown={keepCaretKeys}>
  <SearchField bind:value={query} label={t('chat.model.filter')} placeholder={t('chat.model.filterPlaceholder')} autofocus />

  <div class="flex max-h-72 flex-col gap-2 overflow-y-auto" role="group" aria-label={t('chat.model.group')}>
    {@render leading?.()}

    {#if models === null}
      <p class="px-2 py-3 text-sm text-fg-3" role="status">{unavailable ?? t('chat.model.loading')}</p>
    {:else if groups.length === 0}
      <p class="px-2 py-3 text-sm text-fg-3" role="status">{t('chat.model.none')}</p>
    {/if}

    {#each groups as group (group.provider)}
      <div class="flex flex-col">
        <p class="px-2.5 py-1 text-xs font-medium tracking-wide text-fg-3 uppercase">{group.provider}</p>
        {#each group.models as model (model.id)}
          <MenuItem label={model.name ?? model.id} detail={model.contextWindow ? `${Math.round(model.contextWindow / 1000)}k` : null} selected={sameModel(model)} onselect={() => onchoose(model)} />
        {/each}
      </div>
    {/each}
  </div>
</div>
