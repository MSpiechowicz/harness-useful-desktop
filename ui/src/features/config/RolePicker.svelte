<script lang="ts">
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { ipc } from '$lib/ipc/client';
  import { project } from '$lib/stores/project.svelte';

  import type { HarnessModels, ModelScope } from '../routines/harness-models.svelte';
  import type { SelectorRules } from './role-selectors';

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
    /** The model the row has now, filled in for the person to edit. */
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
  // The harness is asked in the open project: what it offers can depend on the project's settings. With no project open
  // it is asked in the routines workspace, the folder agents run in when no project is theirs.
  let workspace = $state<string | null>(null);
  const folder = $derived(project.active?.path ?? workspace);
  const scope = $derived<ModelScope | null>(folder ? { harness, project: folder } : null);
  const offered = $derived(scope ? catalog.modelsOf(scope) : null);
  const loading = $derived(scope !== null && offered === null && !catalog.failed(scope));
  const failed = $derived(scope !== null && catalog.failed(scope));
  const available = $derived((offered ?? []).map((model) => rules.selectorOf(model)).filter((text) => rules.isValid(text)));
  const choices = $derived([...new Set([...rules.aliases, ...suggestions, ...available])]);
  // A model that is still the current one filters nothing: the person came to pick another.
  const needle = $derived(typed === current ? '' : typed.toLowerCase());
  const shown = $derived(choices.filter((text) => text.toLowerCase().includes(needle)));
  const invalid = $derived(typed !== '' && !rules.isValid(typed));

  // Each opening starts from the model the row has now.
  $effect(() => {
    if (open) {
      value = untrack(() => current);
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
    } else if (!project.active && workspace === null) {
      void findWorkspace();
    }
  });

  async function findWorkspace(): Promise<void> {
    try {
      workspace = await ipc.invoke('routines_workspace', undefined);
    } catch {
      // No folder to ask in: the model is typed.
    }
  }

  async function submit(selector: string): Promise<void> {
    if (busy || !rules.isValid(selector)) {
      return;
    }

    busy = true;
    const done = await onpick(role, selector);
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

<Popover label={t('components.configView.roles.panel', { role })} kind="dialog" align="end" side="auto" matchTrigger={false} bind:open class="w-96">
  {#snippet trigger({ props })}
    <!-- Mapping is the one thing to do on a role that has no model, so it is the primary button. Changing one stays quiet. -->
    <Button variant={kind === 'map' ? 'primary' : 'ghost'} {...props} aria-label={t(words.label, { role })}>{t(words.button)}</Button>
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
      <p class="px-1 text-xs text-fg-3">{t(words.hint)}</p>
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
