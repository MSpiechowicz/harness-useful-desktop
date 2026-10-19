<script lang="ts">
  import X from '@lucide/svelte/icons/x';

  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import { cx } from '$lib/utils/cx';

  import { ICON } from '../icon';
  import { type ChipRules, chipsOf, mergeChips } from './chips';
  import { fieldClass } from './doc-field';

  interface Props {
    /** The id of the text field, which the label of the `DocField` names. */
    id: string;
    /** The chips so far. */
    values: string[];
    /** The text typed since the last chip. It counts as a chip once the field is left, so the owner may read it too. */
    draft: string;
    rules: ChipRules;
    /** Names the button that removes a chip, with the chip as `{value}`. */
    removeLabel: MessageKey;
    placeholder?: string;
    /** Chips in a monospace face, for names of files and code. */
    mono?: boolean;
    /** A chip that is not acceptable is shown in red. */
    flagged?: (value: string) => boolean;
    invalid?: boolean;
    onblur?: () => void;
  }

  let { id, values = $bindable(), draft = $bindable(), rules, removeLabel, placeholder, mono = false, flagged, invalid = false, onblur }: Props = $props();

  let input = $state<HTMLInputElement>();

  const chipClass = $derived(mono ? 'rounded-sm px-2 font-mono' : 'rounded-full pr-1.5 pl-2.5');

  function add(chips: string[]): void {
    values = mergeChips(values, chips);
  }

  /** Typing a separator ends a chip: what is before it becomes one, what follows stays to be typed. */
  function typed(event: Event & { currentTarget: HTMLInputElement }): void {
    const pieces = event.currentTarget.value.split(rules.split);
    const rest = pieces.pop() ?? '';

    if (pieces.length === 0) {
      return;
    }

    add(pieces.map(rules.normalize).filter((piece) => piece !== ''));
    draft = rest;
    event.currentTarget.value = rest;
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' && draft.trim() !== '') {
      event.preventDefault();
      commit();
    } else if (event.key === 'Backspace' && draft === '') {
      values = values.slice(0, -1);
    }
  }

  /** A pasted list becomes chips at once. */
  function paste(event: ClipboardEvent): void {
    const text = event.clipboardData?.getData('text') ?? '';

    if (!rules.split.test(text)) {
      return;
    }

    event.preventDefault();
    add(chipsOf(draft + text, rules));
    draft = '';
  }

  function commit(): void {
    add(chipsOf(draft, rules));
    draft = '';
  }

  function remove(value: string): void {
    values = values.filter((chip) => chip !== value);
    input?.focus();
  }
</script>

<!-- A click anywhere in the field goes to the text field, which is the one thing here to focus. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class={fieldClass(invalid, 'min-h-control px-2 py-1', 'flex flex-wrap items-center gap-1.5 focus-within:border-focus')} onclick={() => input?.focus()}>
  {#each values as value (value)}
    <span class={cx('inline-flex items-center gap-1 py-0.5 text-xs', flagged?.(value) ? 'bg-danger-wash text-danger' : 'bg-surface-3 text-fg-2', chipClass)}>
      {value}
      <button
        type="button"
        aria-label={t(removeLabel, { value })}
        class="grid place-items-center rounded-full text-fg-3 transition-colors duration-[var(--dur-fast)] hover:text-fg"
        onclick={() => remove(value)}
      >
        <X size={ICON.sm} aria-hidden="true" />
      </button>
    </span>
  {/each}

  <input
    {id}
    bind:this={input}
    bind:value={draft}
    oninput={typed}
    onkeydown={keydown}
    onpaste={paste}
    onblur={() => {
      commit();
      onblur?.();
    }}
    placeholder={values.length === 0 ? placeholder : undefined}
    autocomplete="off"
    spellcheck="false"
    aria-invalid={invalid ? 'true' : undefined}
    class="h-6 min-w-24 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-fg-3"
  />
</div>
