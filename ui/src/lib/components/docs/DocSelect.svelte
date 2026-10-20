<script lang="ts" generics="V extends string">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import { cx } from '$lib/utils/cx';

  import { ICON, type IconComponent } from '../icon';
  import MenuSearch from '../MenuSearch.svelte';
  import { matchOptions, type MenuSearchTexts } from '../menu-search';
  import Popover from '../Popover.svelte';

  interface Props {
    /** Names the menu the choices open in. */
    label: string;
    options: { value: V; label: string; hint?: string; icon?: IconComponent }[];
    value: V;
    /** The id of the visible label that names the field. */
    labelledby: string;
    onchange: (value: V) => void;
    /** Shown while no option is chosen (the value is not among them). Without it the first option stands in. */
    placeholder?: string;
    /** A field above the choices that keeps those whose label or hint has what was typed; for a field with many. The list then has a fixed height with its own scroll. */
    search?: MenuSearchTexts;
    /** The choice is fixed: the field shows it and does not open. */
    disabled?: boolean;
  }

  let { label, options, value, labelledby, onchange, placeholder, search, disabled = false }: Props = $props();

  let needle = $state('');
  const shown = $derived(matchOptions(options, needle));

  const chosen = $derived(options.find((option) => option.value === value));
  const current = $derived(chosen ?? (placeholder === undefined ? options[0] : undefined));
  const CurrentIcon = $derived(current?.icon);
</script>

<!-- A few moments or modes to choose from, each with a line on what it means. It opens over the form, so the choice is read in place. -->
<Popover {label} kind="menu" side="auto" rootClass="w-full" class="w-full min-w-0!">
  {#snippet trigger({ props })}
    <button
      {...props}
      type="button"
      {disabled}
      aria-labelledby={labelledby}
      class="flex h-control w-full items-center gap-2.5 rounded-md border border-line-strong bg-surface-2 px-3 text-left text-base text-fg transition-colors duration-[var(--dur-fast)] hover:bg-surface-3 focus-visible:border-focus disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-surface-2"
    >
      {#if CurrentIcon}<CurrentIcon size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />{/if}
      {#if current}
        <span class="shrink-0">{current.label}</span>
        <span class="min-w-0 flex-1 truncate text-sm text-fg-3">{current.hint ?? ''}</span>
      {:else}
        <span class="min-w-0 flex-1 truncate text-fg-3">{placeholder}</span>
      {/if}
      {#if !disabled}<ChevronDown size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />{/if}
    </button>
  {/snippet}

  {#snippet children({ close })}
    <MenuSearch {search} bind:needle none={shown.length === 0}>
      {#each shown as option (option.value)}
        {@const Icon = option.icon}

        <button
          type="button"
          role="menuitemradio"
          aria-checked={option.value === value}
          data-menu-item
          class="flex items-center gap-2.5 rounded-sm px-2.5 py-2 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover"
          onclick={() => {
            close();
            onchange(option.value);
          }}
        >
          {#if Icon}<Icon size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />{/if}
          <span class="flex min-w-0 flex-1 flex-col">
            <span class={cx('truncate text-sm', option.value === value ? 'text-fg' : 'text-fg-2')}>{option.label}</span>
            {#if option.hint}<span class="truncate text-xs text-fg-3">{option.hint}</span>{/if}
          </span>
          {#if option.value === value}<Check size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />{/if}
        </button>
      {/each}
    </MenuSearch>
  {/snippet}
</Popover>
