<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Popover from '$lib/components/Popover.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';

  import MenuItem from '../chat/MenuItem.svelte';
  import type { HarnessModels } from '../routines/harness-models.svelte';
  import { levelsWithSaved } from '../routines/routine-models';
  import { RoleScope } from './role-scope.svelte';
  import { joinSelector, modelToAsk, splitSelector, type SelectorRules } from './role-selectors';
  import ThinkingLevel from './ThinkingLevel.svelte';

  interface Props {
    role: string;
    /** The harness whose levels are offered. */
    harness: HarnessId;
    /** Where the models and their levels are asked, shared by every row of the table. */
    catalog: HarnessModels;
    rules: SelectorRules;
    /** The row's model as written, with its thinking level. */
    current: string;
    /** Normalized search needle; empty highlights nothing. */
    needle?: string;
    /** Saves the row's new selector; false when it failed (the reason is already said). */
    onpick: (role: string, selector: string) => Promise<boolean>;
  }

  let { role, harness, catalog, rules, current, needle = '', onpick }: Props = $props();

  let open = $state(false);
  let busy = $state(false);

  const parts = $derived(splitSelector(rules, current));
  const where = new RoleScope(() => harness);
  const scope = $derived(where.current);
  const listed = $derived(scope ? catalog.modelsOf(scope) : null);
  const model = $derived(modelToAsk(rules, parts.model, listed));
  const option = $derived(model && listed?.find((candidate) => candidate.provider === model.provider && candidate.id === model.id));
  // A model the harness says does not reason has no levels to ask for.
  const reasons = $derived(option?.reasoning !== false);
  // Levels belong to a model. One that names no provider is found in the harness's list first, and one the list does not
  // have (an alias), or every one when the list cannot be had, takes the levels the harness's file accepts for it. A row
  // that names a provider but reads as no model has none.
  const asked = $derived(scope && model ? catalog.levelsOf(scope, model) : null);
  const listKnown = $derived(listed !== null || where.unavailable || (scope !== null && catalog.failed(scope)));
  const levels = $derived.by(() => {
    if (!reasons) {
      return [];
    }

    if (model !== null) {
      return asked;
    }

    if (rules.namesProvider) {
      return [];
    }

    return listKnown ? [...rules.unlistedLevels] : null;
  });
  const failed = $derived(where.unavailable || (scope !== null && model !== null && catalog.failed(scope, model)));
  const choices = $derived(levelsWithSaved(levels ?? [], parts.level ?? ''));

  $effect(() => {
    if (!open) {
      return;
    }

    if (!scope) {
      void where.find();

      return;
    }

    void catalog.loadModels(scope);

    if (model && reasons) {
      void catalog.loadLevels(scope, model);
    }
  });

  async function choose(level: string | null): Promise<void> {
    if (busy) {
      return;
    }

    if (level === parts.level) {
      open = false;

      return;
    }

    busy = true;
    const done = await onpick(role, joinSelector(rules, parts.model, level));
    busy = false;

    if (done) {
      open = false;
    }
  }
</script>

<Popover label={t('components.configView.roles.thinkingPanel', { role })} kind="menu" align="start" side="auto" matchTrigger={false} bind:open class="min-w-44">
  {#snippet trigger({ props })}
    <!-- The level reads as plain text until hovered: the chevron says it opens a menu. -->
    <button
      type="button"
      {...props}
      aria-label={t('components.configView.roles.thinkingLabel', { role, level: parts.level ?? t('components.configView.roles.thinkingDefault') })}
      class="group -ml-2 inline-flex h-8 items-center gap-1.5 rounded-md pr-1.5 pl-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover aria-expanded:bg-hover"
    >
      {#if parts.level === null}
        <span class="text-fg-3">—</span>
      {:else}
        <ThinkingLevel level={parts.level} {needle} />
      {/if}
      <ChevronDown size={14} aria-hidden="true" class="shrink-0 text-fg-3 opacity-0 transition-opacity duration-[var(--dur-fast)] group-hover:opacity-100 group-focus-visible:opacity-100 group-aria-expanded:opacity-100" />
    </button>
  {/snippet}

  {#snippet children()}
    {#if levels === null}
      <p class="flex items-center gap-2 px-2.5 py-2 text-sm text-fg-3" role="status">
        {#if failed}
          {t('components.configView.roles.levelsFailed')}
        {:else}
          <Spinner size={12} />{t('components.configView.roles.loadingLevels')}
        {/if}
      </p>
    {:else if choices.length === 0}
      <p class="px-2.5 py-2 text-sm text-fg-3" role="status">{t('components.configView.roles.noLevels')}</p>
    {/if}

    <MenuItem label={t('components.configView.roles.thinkingDefault')} selected={parts.level === null} disabled={busy} onselect={() => void choose(null)} />
    {#each choices as level (level)}
      <MenuItem label={level} selected={level === parts.level} disabled={busy} onselect={() => void choose(level)} />
    {/each}
  {/snippet}
</Popover>
