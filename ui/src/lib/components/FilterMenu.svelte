<script lang="ts" generics="V extends string">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ListFilter from '@lucide/svelte/icons/list-filter';
  import X from '@lucide/svelte/icons/x';

  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import Button from './Button.svelte';
  import { ICON, type IconComponent } from './icon';
  import MenuSearch from './MenuSearch.svelte';
  import { matchOptions, type MenuSearchTexts } from './menu-search';
  import Popover from './Popover.svelte';

  interface Props {
    /** Accessible name of the menu, e.g. "Show". */
    label: string;
    /** The predefined filters; the first one is the unfiltered "All". A `count` is shown after the label, a `hint` under it in quiet text (the search matches it too). A `disabled` one is shown and reachable by keyboard (aria-disabled) but cannot be chosen, and says why in grey under its label when it has a `reason` (read out as its description). */
    options: { value: V; label: string; hint?: string; icon?: IconComponent; count?: number; disabled?: boolean; reason?: string }[];
    value: V;
    onchange: (value: V) => void;
    /** Names what the button filters ("When"), so it reads "When: All". The button then shows the filter on, and a clear x when it narrows the list. */
    prefix?: string;
    /** The icon of the button whatever is chosen; by default the chosen option's own icon, or a filter funnel. `null` draws none, for a plain dropdown. */
    icon?: IconComponent | null;
    /** Takes the filter back to the first option; given with `prefix`, it puts the clear x in the button. */
    onclear?: () => void;
    /** The accessible name of the clear x, e.g. "Clear the When filter". */
    clearLabel?: string;
    /** Which edge of the button the menu lines up with; `end` for a button at the right of a header, `start` at the left. */
    align?: 'start' | 'end';
    /** A menu for long labels (folder names): wider than the usual one. */
    wide?: boolean;
    /** A field above the choices that keeps those whose label or hint has what was typed; for a menu with many. The list then has a fixed height with its own scroll. The texts are its accessible name, hint and the line shown when nothing matches. */
    search?: MenuSearchTexts;
    /** Makes it a menu of actions instead of a filter: the button says what choosing does ("Link a repository") and shows no chosen value, and the choices are plain items that `onchange` is told of. `value` is then unused. */
    action?: { label: string; icon: IconComponent };
    /** Keeps the button one width whatever is chosen: every label is stacked in one cell, so the longest sets the width and picking another shifts nothing beside it. */
    steady?: boolean;
    /** The button fills the width it is given, its label at the start and the chevron at the end, and the menu matches it in width and text size. */
    fill?: boolean;
    /** Which side of the button the menu opens on; `auto` for a button inside a scrolling form or dialog, which would cut the menu off. */
    side?: 'top' | 'bottom' | 'auto';
    /** Turns an action menu off: its button cannot be pressed. The caller says why next to it. */
    disabled?: boolean;
    /** Names the text that says why the menu is `disabled`, read out as the button's description. */
    describedby?: string;
  }

  let { label, options, value, onchange, prefix, icon, onclear, clearLabel, align = 'end', wide = false, search, action, steady = false, fill = false, side = 'bottom', disabled = false, describedby }: Props = $props();

  const reasonId = uid('filter-reason');
  let needle = $state('');
  const shown = $derived(matchOptions(options, needle));

  const current = $derived(options.find((option) => option.value === value) ?? options[0]);
  const TriggerIcon = $derived(icon === null ? null : (icon ?? current?.icon ?? ListFilter));
  const narrowing = $derived(options.length > 0 && value !== options[0].value);
  const counted = $derived(options.some((option) => option.count !== undefined));
</script>

<!-- The chosen label. A steady button stacks all of them in one cell and shows only the chosen one. -->
{#snippet chosen()}
  {#if steady}
    <span class="inline-grid min-w-0 align-bottom">
      {#each options as option (option.value)}
        <span class={cx('truncate text-left [grid-area:1/1]', option.value !== current.value && 'invisible')} aria-hidden="true">{option.label}</span>
      {/each}
    </span>
  {:else}
    {current.label}
  {/if}
{/snippet}

{#snippet triggerIcon()}
  {#if TriggerIcon}<TriggerIcon size={ICON.sm} aria-hidden="true" />{/if}
{/snippet}

<!-- A filter picks from a few ready-made choices; typing is for search. Its button does not shrink when pressed, unlike an action's. -->
<Popover {label} kind="menu" {align} {side} matchTrigger={!action} rootClass={fill ? 'w-full' : undefined} class={cx('min-w-0!', fill ? 'w-full' : search ? 'w-72' : counted || wide ? 'w-60' : 'w-48')}>
  {#snippet trigger({ props, open })}
    {#if action}
      <!-- Off but still focusable (aria-disabled), so a keyboard or screen-reader user reaches the button and hears why. -->
      <Button
        {...props}
        onclick={disabled ? undefined : props.onclick}
        aria-disabled={disabled ? 'true' : undefined}
        aria-describedby={disabled ? describedby : undefined}
        class={cx(disabled && 'cursor-not-allowed opacity-50 hover:bg-surface-2 active:scale-100')}
      >
        {#snippet leading()}<action.icon size={ICON.sm} aria-hidden="true" />{/snippet}
        {action.label}
      </Button>
    {:else if prefix}
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
          class="inline-flex h-full items-center gap-2 rounded-md pr-2.5 pl-3 text-base font-medium whitespace-nowrap text-fg"
        >
          {#if TriggerIcon}<TriggerIcon size={ICON.sm} class="text-fg-3" aria-hidden="true" />{/if}
          <span><span class={narrowing ? 'font-normal text-fg-2' : 'font-normal text-fg-3'}>{prefix}:</span> <span class={narrowing ? 'text-accent-text' : undefined}>{@render chosen()}</span></span>
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
      <Button {...props} aria-label={`${label}: ${current.label}`} leading={TriggerIcon ? triggerIcon : undefined} class={cx('active:scale-100!', fill && 'w-full justify-between!')}>
        {@render chosen()}
        {#snippet trailing()}<ChevronDown size={ICON.sm} aria-hidden="true" />{/snippet}
      </Button>
    {/if}
  {/snippet}

  {#snippet children({ close })}
    <MenuSearch {search} bind:needle none={shown.length === 0}>
      {#each shown as option, index (option.value)}
        {@const why = option.disabled ? option.reason : undefined}
        <button
          type="button"
          role={action ? 'menuitem' : 'menuitemradio'}
          aria-checked={action ? undefined : option.value === value}
          aria-disabled={option.disabled ? 'true' : undefined}
          aria-describedby={why ? `${reasonId}-${index}` : undefined}
          data-menu-item=""
          class={cx(
            'flex items-center justify-between gap-2 rounded-sm px-2.5 py-2 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover',
            fill ? 'text-base' : 'text-sm',
            option.value === value ? 'text-fg' : 'text-fg-2',
            option.disabled && 'cursor-not-allowed hover:bg-transparent',
            option.disabled && !why && 'opacity-50',
          )}
          onclick={() => {
            // An off choice stays in the keyboard order so its reason is heard, and choosing it does nothing.
            if (option.disabled) {
              return;
            }

            close();
            onchange(option.value);
          }}
        >
          <span class="flex min-w-0 items-center gap-2.5">
            {#if option.icon}<option.icon size={ICON.sm} class={cx('shrink-0 text-fg-3', why && 'opacity-50')} aria-hidden="true" />{/if}
            {#if why}
              <span class="flex min-w-0 flex-col"><span class="truncate opacity-50">{option.label}</span> <span id="{reasonId}-{index}" class="text-xs text-fg-3">{why}</span></span>
            {:else if option.hint}
              <span class="flex min-w-0 flex-col"><span class="truncate">{option.label}</span> <span class="truncate text-xs text-fg-3">{option.hint}</span></span>
            {:else}
              <span class="truncate">{option.label}</span>
            {/if}
          </span>
          <span class="flex shrink-0 items-center gap-2">
            {#if option.count !== undefined}<span class={cx('tabular min-w-[2ch] text-right text-xs text-fg-3', option.count === 0 && 'opacity-60')}>{option.count}</span>{/if}
            <!-- The tick has a slot of its own in every row, so the counts line up whether or not one is chosen. -->
            {#if !action && (counted || option.value === value)}
              <span class="grid size-4 shrink-0 place-items-center" aria-hidden="true">
                {#if option.value === value}<Check size={ICON.sm} class="text-accent-text" />{/if}
              </span>
            {/if}
          </span>
        </button>
      {/each}
    </MenuSearch>
  {/snippet}
</Popover>
