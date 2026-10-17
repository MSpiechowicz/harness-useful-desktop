<script lang="ts" generics="V extends string">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ListFilter from '@lucide/svelte/icons/list-filter';

  import { cx } from '$lib/utils/cx';

  import Button from './Button.svelte';
  import { ICON, type IconComponent } from './icon';
  import Popover from './Popover.svelte';

  interface Props {
    /** Accessible name of the menu, e.g. "Show". */
    label: string;
    /** The predefined filters; the first one is the unfiltered "All". */
    options: { value: V; label: string; icon?: IconComponent }[];
    value: V;
    onchange: (value: V) => void;
    /** Which edge of the button the menu lines up with; `end` for a button at the right of a header, `start` at the left. */
    align?: 'start' | 'end';
  }

  let { label, options, value, onchange, align = 'end' }: Props = $props();

  const current = $derived(options.find((option) => option.value === value) ?? options[0]);
  const TriggerIcon = $derived(current.icon ?? ListFilter);
</script>

<!-- A filter picks from a few ready-made choices; typing is for search. -->
<Popover {label} kind="menu" {align} class="w-48 min-w-0!">
  {#snippet trigger({ props })}
    <Button {...props} aria-label={`${label}: ${current.label}`}>
      {#snippet leading()}<TriggerIcon size={ICON.sm} aria-hidden="true" />{/snippet}
      {current.label}
      {#snippet trailing()}<ChevronDown size={ICON.sm} aria-hidden="true" />{/snippet}
    </Button>
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
          {#if option.value === value}<Check size={ICON.sm} class="text-accent-text" aria-hidden="true" />{/if}
        </button>
      {/each}
    </div>
  {/snippet}
</Popover>
