<script lang="ts" generics="V extends string">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ListFilter from '@lucide/svelte/icons/list-filter';
  import X from '@lucide/svelte/icons/x';

  import { cx } from '$lib/utils/cx';

  import Button from './Button.svelte';
  import { ICON, type IconComponent } from './icon';
  import Popover from './Popover.svelte';

  interface Props {
    /** Accessible name of the menu, e.g. "Show". */
    label: string;
    /** The predefined filters; the first one is the unfiltered "All". A `count` is shown after the label. */
    options: { value: V; label: string; icon?: IconComponent; count?: number }[];
    value: V;
    onchange: (value: V) => void;
    /** Names what the button filters ("When"), so it reads "When: All". The button then shows the filter on, and a clear x when it narrows the list. */
    prefix?: string;
    /** The icon of the button whatever is chosen; by default the chosen option's own icon, or a filter funnel. */
    icon?: IconComponent;
    /** Takes the filter back to the first option; given with `prefix`, it puts the clear x in the button. */
    onclear?: () => void;
    /** The accessible name of the clear x, e.g. "Clear the When filter". */
    clearLabel?: string;
    /** Which edge of the button the menu lines up with; `end` for a button at the right of a header, `start` at the left. */
    align?: 'start' | 'end';
    /** A menu for long labels (folder names): wider than the usual one. */
    wide?: boolean;
  }

  let { label, options, value, onchange, prefix, icon, onclear, clearLabel, align = 'end', wide = false }: Props = $props();

  const current = $derived(options.find((option) => option.value === value) ?? options[0]);
  const TriggerIcon = $derived(icon ?? current.icon ?? ListFilter);
  const narrowing = $derived(options.length > 0 && value !== options[0].value);
  const counted = $derived(options.some((option) => option.count !== undefined));
</script>

<!-- A filter picks from a few ready-made choices; typing is for search. -->
<Popover {label} kind="menu" {align} class={cx('min-w-0!', counted || wide ? 'w-60' : 'w-48')}>
  {#snippet trigger({ props, open })}
    {#if prefix}
      <!-- The button and its clear x share one frame, so the pair reads as one control: the x clears, the rest opens the menu. -->
      <div
        class={cx(
          'inline-flex h-control items-center rounded-md border transition-colors duration-[var(--dur-fast)]',
          narrowing ? 'border-accent/30 bg-accent-wash' : 'border-line-strong bg-surface-2 shadow-raised hover:bg-surface-3',
        )}
      >
        <button
          {...props}
          type="button"
          aria-label={`${prefix}: ${current.label}`}
          class="inline-flex h-full items-center gap-2 rounded-md pr-2.5 pl-3 text-base font-medium whitespace-nowrap text-fg transition-transform duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] active:scale-[0.97]"
        >
          <TriggerIcon size={ICON.sm} class="text-fg-3" aria-hidden="true" />
          <span><span class={narrowing ? 'font-normal text-fg-2' : 'font-normal text-fg-3'}>{prefix}:</span> <span class={narrowing ? 'text-accent-text' : undefined}>{current.label}</span></span>
          <ChevronDown size={ICON.sm} class={cx('text-fg-3 transition-transform duration-[var(--dur-base)] ease-[var(--ease-out-expo)]', open && 'rotate-180')} aria-hidden="true" />
        </button>

        {#if narrowing && onclear}
          <button
            type="button"
            aria-label={clearLabel}
            class="mr-1 grid size-7 place-items-center rounded-sm text-fg-2 transition-[transform,background-color,color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] hover:bg-press hover:text-fg active:scale-90"
            onclick={onclear}
          >
            <X size={ICON.sm} strokeWidth={1.8} aria-hidden="true" />
          </button>
        {/if}
      </div>
    {:else}
      <Button {...props} aria-label={`${label}: ${current.label}`}>
        {#snippet leading()}<TriggerIcon size={ICON.sm} aria-hidden="true" />{/snippet}
        {current.label}
        {#snippet trailing()}<ChevronDown size={ICON.sm} aria-hidden="true" />{/snippet}
      </Button>
    {/if}
  {/snippet}

  {#snippet children({ close })}
    <div role="none" class="flex flex-col">
      {#each options as option (option.value)}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={option.value === value}
          data-menu-item
          class={cx(
            'flex items-center justify-between gap-2 rounded-sm px-2.5 py-2 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover',
            option.value === value ? 'text-fg' : 'text-fg-2',
          )}
          onclick={() => {
            close();
            onchange(option.value);
          }}
        >
          <span class="flex min-w-0 items-center gap-2.5">
            {#if option.icon}<option.icon size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />{/if}
            <span class="truncate">{option.label}</span>
          </span>
          <span class="flex shrink-0 items-center gap-2">
            {#if option.count !== undefined}<span class={cx('tabular text-xs text-fg-3', option.count === 0 && 'opacity-60')}>{option.count}</span>{/if}
            {#if option.value === value}
              <Check size={ICON.sm} class="text-accent-text" aria-hidden="true" />
            {:else if counted}
              <span class="size-4" aria-hidden="true"></span>
            {/if}
          </span>
        </button>
      {/each}
    </div>
  {/snippet}
</Popover>
