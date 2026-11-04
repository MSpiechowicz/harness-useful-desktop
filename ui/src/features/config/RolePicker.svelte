<script lang="ts">
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';

  import type { HarnessModels } from '../routines/harness-models.svelte';
  import { RoleScope } from './role-scope.svelte';
  import { joinSelector, modelToAsk, splitSelector, type SelectorRules } from './role-selectors';

  type Kind = 'map' | 'change';

  interface Props {
    /** Map: the model of a role the file does not have yet. Change: the model of a row the file has. */
    kind: Kind;
    role: string;
    /** The harness whose models are offered. */
    harness: HarnessId;
    /** Where the models the harness offers are asked, once for every picker of the table. */
    catalog: HarnessModels;
    rules: SelectorRules;
    /** The row's model as written, with its thinking level. Its model is filled in for the person to edit. */
    current?: string;
    /** Models the file already uses, offered after the harness's own values and before every model it offers. */
    suggestions: string[];
    /** Saves the model; false when it failed (the reason is already said). */
    onpick: (role: string, selector: string) => Promise<boolean>;
  }

  let { kind, role, harness, catalog, rules, current = '', suggestions, onpick }: Props = $props();

  const WORDS: Record<Kind, { button: MessageKey; label: MessageKey; hint: MessageKey }> = {
    map: { button: 'components.configView.roles.map', label: 'components.configView.roles.mapLabel', hint: 'components.configView.roles.mapHint' },
    change: { button: 'components.configView.roles.change', label: 'components.configView.roles.changeLabel', hint: 'components.configView.roles.changeHint' },
  };

  let open = $state(false);
  let value = $state('');
  let busy = $state(false);

  const words = $derived(WORDS[kind]);
  const typed = $derived(value.trim());
  // The row's level stays with the new model, so the field holds only the model.
  const parts = $derived(splitSelector(rules, current));
  const where = new RoleScope(() => harness);
  const scope = $derived(where.current);
  const offered = $derived(scope ? catalog.modelsOf(scope) : null);
  const loading = $derived(scope !== null && offered === null && !catalog.failed(scope));
  const failed = $derived(scope !== null && catalog.failed(scope));
  const available = $derived((offered ?? []).map((model) => rules.selectorOf(model)).filter((text) => rules.isValid(text)));
  const choices = $derived([...new Set([...rules.aliases, ...suggestions, ...available])]);
  // A model that is still the current one filters nothing: the person came to pick another.
  const needle = $derived(typed === parts.model ? '' : typed.toLowerCase());
  const shown = $derived(choices.filter((text) => text.toLowerCase().includes(needle)));
  const invalid = $derived(typed !== '' && !rules.isValid(typed));

  // Each opening starts from the model the row has now.
  $effect(() => {
    if (open) {
      value = untrack(() => parts.model);
    }
  });

  // While open, the harness is asked for its models in the folder of the moment: a project opened meanwhile is asked too.
  // Asking again for a folder already answered, or being asked, costs nothing.
  $effect(() => {
    if (!open) {
      return;
    }

    if (scope) {
      void catalog.loadModels(scope);
    } else {
      void where.find();
    }
  });

  /** `selector` with the row's level when it names none and the new model offers that level. */
  async function withKeptLevel(selector: string): Promise<string> {
    const picked = splitSelector(rules, selector);

    if (parts.level === null || picked.level !== null || rules.levelless.includes(picked.model)) {
      return selector;
    }

    // A quick pick can come before the folder or the harness's list is known: both are waited for.
    const asked = await where.settled();

    if (asked) {
      await catalog.loadModels(asked);
    }

    // Read after the waits: the Thinking menu may have changed the level meanwhile.
    const level = parts.level;

    if (level === null) {
      return selector;
    }

    const listed = asked ? catalog.modelsOf(asked) : null;
    const model = modelToAsk(rules, picked.model, listed);

    if (model === null) {
      return rules.unlistedLevels.includes(level) ? joinSelector(rules, picked.model, level) : selector;
    }

    const option = listed?.find((candidate) => candidate.provider === model.provider && candidate.id === model.id);

    // A model the harness does not reason with, or does not list at all, is not asked about: the level goes.
    if (!asked || option?.reasoning === false || (listed !== null && option === undefined)) {
      return selector;
    }

    await catalog.loadLevels(asked, model);

    // The level may have been changed in the Thinking menu meanwhile: decide again for the new one.
    if (parts.level !== level) {
      return withKeptLevel(selector);
    }

    return catalog.levelsOf(asked, model)?.includes(level) ? joinSelector(rules, picked.model, level) : selector;
  }

  async function submit(selector: string): Promise<void> {
    if (busy || !rules.isValid(selector)) {
      return;
    }

    busy = true;
    const done = await onpick(role, await withKeptLevel(selector));
    busy = false;

    if (done) {
      open = false;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submit(typed);
    } else if (event.key === 'Home' || event.key === 'End') {
      // They move the caret here, not the list.
      event.stopPropagation();
    }
  }
</script>

<Popover label={t('components.configView.roles.panel', { role })} kind="dialog" align="end" side="auto" matchTrigger={false} bind:open class="w-96" rootClass="w-full">
  {#snippet trigger({ props })}
    <!-- Map and Change are the same kind of button, as wide as the actions column, so they line up and none outshouts the rows. -->
    <Button variant="secondary" class="w-full" {...props} aria-label={t(words.label, { role })}>{t(words.button)}</Button>
  {/snippet}

  <div class="flex flex-col gap-2">
    <input
      bind:value
      type="text"
      data-autofocus
      aria-label={t('components.configView.roles.field', { role })}
      aria-invalid={invalid}
      placeholder={t(rules.placeholder)}
      autocomplete="off"
      spellcheck="false"
      disabled={busy}
      class="h-control w-full rounded-md border border-line-strong bg-surface-2 px-3 font-mono text-sm text-fg outline-none placeholder:text-fg-3 focus-visible:border-accent/50"
      onkeydown={onKeydown}
    />

    {#if invalid}
      <p class="px-1 text-xs text-fg-3" role="status">{t(rules.invalid)}</p>
    {:else}
      <p class="px-1 text-xs text-fg-3">
        {kind === 'change' && parts.level !== null ? t('components.configView.roles.changeHintKeeps', { level: parts.level }) : t(words.hint)}
      </p>
    {/if}

    <!-- A list to pick from, not a choice that stays ticked, so the rows start at the field's edge with no check column. -->
    {#if shown.length > 0}
      <div class="flex max-h-56 flex-col overflow-y-auto" role="group" aria-label={t('components.configView.roles.suggestions')}>
        {#each shown as text (text)}
          <button
            type="button"
            data-menu-item
            title={text}
            disabled={busy}
            class="w-full shrink-0 truncate rounded-sm px-3 py-1.5 text-left font-mono text-sm text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg focus-visible:bg-hover focus-visible:text-fg focus-visible:-outline-offset-2 active:bg-press"
            onclick={() => void submit(text)}
          >
            {text}
          </button>
        {/each}
      </div>
    {/if}

    {#if loading}
      <p class="flex items-center gap-2 px-3 text-xs text-fg-3" role="status"><Spinner size={12} />{t('components.configView.roles.loadingModels')}</p>
    {:else if failed}
      <p class="px-3 text-xs text-fg-3" role="status">{t('components.configView.roles.modelsFailed')}</p>
    {/if}
  </div>
</Popover>
