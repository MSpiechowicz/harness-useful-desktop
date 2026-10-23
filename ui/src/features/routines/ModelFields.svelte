<script lang="ts">
  import { untrack } from 'svelte';
  import { slide } from 'svelte/transition';

  import DocField from '$lib/components/docs/DocField.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { uid } from '$lib/utils/ids';

  import MenuItem from '../chat/MenuItem.svelte';
  import ModelChoices from '../chat/ModelChoices.svelte';
  import FieldMenu from './FieldMenu.svelte';
  import type { HarnessModels, ModelScope } from './harness-models.svelte';
  import { levelsWithSaved, sameModel, withSaved } from './routine-models';
  import { type RoutineForm, withModel } from './routine-form';
  import { routines } from './routines-data.svelte';

  interface Props {
    form: RoutineForm;
    /** What is wrong with the model, said in red. */
    error: string | null;
    /** The models and levels the harness offers, asked of a helper agent and kept while the editor is open. */
    catalog: HarnessModels;
  }

  let { form = $bindable(), error, catalog }: Props = $props();

  const ids = { model: uid('routine-model'), thinking: uid('routine-thinking') };

  let modelOpen = $state(false);
  let thinkingOpen = $state(false);

  // The folder the helper works in: the project of a project routine, else where global routines run (or, before one has run, the open project).
  const folder = $derived(form.projectPath ?? routines.workspace ?? project.active?.path ?? null);
  const scope = $derived<ModelScope | null>(folder === null ? null : { harness: form.harness, project: folder });
  const chosen = $derived(form.provider === '' ? null : { provider: form.provider, id: form.modelId });

  const listed = $derived(scope === null ? null : catalog.modelsOf(scope));
  const options = $derived(listed === null ? null : withSaved(listed, chosen));
  const option = $derived(options?.find((model) => sameModel(model, chosen)) ?? null);
  const modelUnavailable = $derived(scope === null || catalog.failed(scope) ? t('routines.editor.model.unavailable') : null);

  // Levels belong to a model and exist only for one that reasons. A model the harness does not list is asked about anyway.
  const reasons = $derived(chosen !== null && option?.reasoning !== false);
  const levels = $derived(scope === null || chosen === null ? null : catalog.levelsOf(scope, chosen));
  // An unknown level is shown only when there is a list to pick from, or the routine sets one: otherwise the field would only say "default".
  const thinkingShown = $derived(reasons && (form.thinking !== '' || (levels?.length ?? 0) > 0));
  const thinkingUnavailable = $derived(scope !== null && chosen !== null && catalog.failed(scope, chosen) ? t('routines.editor.thinking.unavailable') : null);

  $effect(() => {
    if (modelOpen && scope) {
      const asked = scope;

      untrack(() => void catalog.loadModels(asked));
    }
  });

  $effect(() => {
    if (reasons && scope && chosen) {
      const asked = scope;
      const model = chosen;

      untrack(() => void catalog.loadLevels(asked, model));
    }
  });
</script>

<DocField label={t('routines.field.model')} labelId={ids.model} aside={t('routines.editor.optional')} error={error} help={t('routines.editor.model.hint')}>
  <FieldMenu label={t('chat.model.popover')} kind="dialog" labelledby={ids.model} bind:open={modelOpen}>
    {#snippet value()}
      {#if chosen}
        <span class="min-w-0 truncate font-mono text-sm">{option?.name ?? chosen.id}</span>
        <span class="min-w-0 flex-1 truncate text-sm text-fg-3">{chosen.provider}</span>
      {:else}
        <span class="min-w-0 flex-1 truncate">{t('routines.value.defaultModel')}</span>
      {/if}
    {/snippet}

    {#snippet choices({ close })}
      <ModelChoices
        models={options}
        current={chosen}
        unavailable={modelUnavailable}
        onchoose={(model) => {
          form = withModel(form, model);
          close();
        }}
      >
        {#snippet leading()}
          <MenuItem
            label={t('routines.value.defaultModel')}
            selected={chosen === null}
            onselect={() => {
              form = withModel(form, null);
              close();
            }}
          />
        {/snippet}
      </ModelChoices>
    {/snippet}
  </FieldMenu>
</DocField>

{#if thinkingShown}
  <div transition:slide={{ duration: scaled(200) }} class="-m-1 overflow-hidden p-1">
    <DocField label={t('routines.field.thinking')} labelId={ids.thinking} aside={t('routines.editor.optional')} help={t('routines.editor.thinking.hint')}>
      <FieldMenu label={t('chat.thinking.picker')} labelledby={ids.thinking} bind:open={thinkingOpen}>
        {#snippet value()}
          <span class="min-w-0 flex-1 truncate">{form.thinking || t('routines.editor.thinking.default')}</span>
        {/snippet}

        {#snippet choices({ close })}
          {#if levels === null}
            <p class="px-2.5 py-2 text-sm text-fg-3" role="status">{thinkingUnavailable ?? t('chat.thinking.loading')}</p>
          {/if}

          <MenuItem
            label={t('routines.editor.thinking.default')}
            selected={form.thinking === ''}
            onselect={() => {
              form.thinking = '';
              close();
            }}
          />
          {#each levelsWithSaved(levels ?? [], form.thinking) as level (level)}
            <MenuItem
              label={level}
              selected={level === form.thinking}
              onselect={() => {
                form.thinking = level;
                close();
              }}
            />
          {/each}
        {/snippet}
      </FieldMenu>
    </DocField>
  </div>
{/if}
