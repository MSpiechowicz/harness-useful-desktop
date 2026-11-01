<script lang="ts">
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
  import MenuItem from '../chat/MenuItem.svelte';

  import type { SelectorRules } from './role-selectors';

  type Kind = 'map' | 'change';

  interface Props {
    /** Map: the model of a role the file does not have yet. Change: the model of a row the file has. */
    kind: Kind;
    role: string;
    rules: SelectorRules;
    /** The model the row has now, filled in for the person to edit. */
    current?: string;
    /** Models the file already uses, offered as choices after the host's own values. */
    suggestions: string[];
    /** Saves the model; false when it failed (the reason is already said). */
    onpick: (role: string, selector: string) => Promise<boolean>;
  }

  let { kind, role, rules, current = '', suggestions, onpick }: Props = $props();

  const WORDS: Record<Kind, { button: MessageKey; label: MessageKey; hint: MessageKey }> = {
    map: { button: 'components.configView.roles.map', label: 'components.configView.roles.mapLabel', hint: 'components.configView.roles.mapHint' },
    change: { button: 'components.configView.roles.change', label: 'components.configView.roles.changeLabel', hint: 'components.configView.roles.changeHint' },
  };

  let open = $state(false);
  let value = $state('');
  let busy = $state(false);

  const words = $derived(WORDS[kind]);
  const typed = $derived(value.trim());
  const choices = $derived([...new Set([...rules.aliases, ...suggestions])]);
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

<Popover label={t('components.configView.roles.panel', { role })} kind="dialog" align="end" side="auto" matchTrigger={false} bind:open class="w-80">
  {#snippet trigger({ props })}
    <Button variant="ghost" {...props} aria-label={t(words.label, { role })}>{t(words.button)}</Button>
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

    {#if shown.length > 0}
      <div class="flex max-h-56 flex-col overflow-y-auto" role="group" aria-label={t('components.configView.roles.suggestions')}>
        {#each shown as text (text)}
          <MenuItem label={text} onselect={() => void submit(text)} />
        {/each}
      </div>
    {/if}
  </div>
</Popover>
