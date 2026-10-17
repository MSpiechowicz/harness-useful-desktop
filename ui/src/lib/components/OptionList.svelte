<script lang="ts" generics="T extends string">
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';
  import { rovingIndex } from '$lib/utils/roving';

  import { ICON } from './icon';
  import type { StripOption } from './strip';

  interface Props {
    options: StripOption<T>[];
    value: T;
    label: string;
    /** Prefix for the generated ids of the options. */
    idBase: string;
    onchange?: (value: T) => void;
    class?: string;
  }

  let { options, value, label, idBase, onchange, class: className }: Props = $props();

  let group: HTMLElement | undefined = $state();
  let buttons: (HTMLButtonElement | undefined)[] = $state([]);

  function select(option: StripOption<T>): void {
    if (!option.disabled && option.value !== value) {
      onchange?.(option.value);
    }
  }

  // Focus and choice go together: a click moves the focus to the option it chose, and a choice made another way (a filter
  // that was reset) takes the focus along if it was in the list, so the ring never sits on one option with another chosen.
  $effect(() => {
    const chosen = buttons[options.findIndex((option) => option.value === value)];

    if (chosen && group?.contains(document.activeElement) && document.activeElement !== chosen) {
      chosen.focus();
    }
  });

  function onKeydown(event: KeyboardEvent, index: number): void {
    const next = rovingIndex(event.key, index, options.length, 'vertical');

    if (next === null) {
      return;
    }

    event.preventDefault();
    select(options[next]);
    buttons[next]?.focus();
  }
</script>

<!-- The choices of a strip as a column, for a view with room for a sidebar: one radio per line, its count at the end.
     The chosen line is washed in the accent, like the open item of the lists of Config and Extensions. -->
<div bind:this={group} role="radiogroup" aria-label={label} class={cx('flex flex-col gap-0.5', className)}>
  {#each options as option, index (option.value)}
    {@const selected = option.value === value}

    <button
      bind:this={buttons[index]}
      type="button"
      id="{idBase}-option-{option.value}"
      role="radio"
      aria-checked={selected}
      tabindex={selected ? 0 : -1}
      disabled={option.disabled}
      onclick={() => {
        select(option);
        buttons[index]?.focus();
      }}
      onkeydown={(event) => onKeydown(event, index)}
      class={cx(
        'flex h-control w-full items-center gap-2.5 rounded-md border px-2.5 text-left text-sm transition-colors duration-[var(--dur-fast)] focus-visible:outline-offset-[-2px] disabled:cursor-not-allowed disabled:opacity-50',
        selected ? 'border-accent/30 bg-accent-wash text-fg' : 'border-transparent text-fg-2 hover:bg-hover hover:text-fg active:bg-press',
      )}
    >
      {#if option.icon}
        {@const Icon = option.icon}
        <Icon size={ICON.sm} class="shrink-0" aria-hidden="true" />
      {/if}

      <span class="min-w-0 flex-1 truncate">{option.label}</span>

      {#if option.count !== undefined}
        <span class={cx('tabular shrink-0 text-xs transition-colors duration-[var(--dur-fast)]', selected ? 'text-fg-2' : 'text-fg-3')}>{formatCount(option.count)}</span>
      {/if}
    </button>
  {/each}
</div>
