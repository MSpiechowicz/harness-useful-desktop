<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import { ICON } from '$lib/components/icon';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookEvent } from '$lib/ipc/bindings/HookEvent';
  import { cx } from '$lib/utils/cx';

  import { EVENT_HINTS, EVENT_ICONS, EVENT_LABELS, EVENTS } from './hook-format';

  interface Props {
    value: HookEvent;
    /** The id of the visible label that names the field. */
    labelledby: string;
    onchange: (event: HookEvent) => void;
  }

  let { value, labelledby, onchange }: Props = $props();

  const CurrentIcon = $derived(EVENT_ICONS[value]);
</script>

<!-- The five moments a hook can fire at, each with a line on when it happens. It opens over the form, so the choice is read in place. -->
<Popover label={t('hooks.field.when')} kind="menu" rootClass="w-full" class="w-full min-w-0!">
  {#snippet trigger({ props })}
    <button
      {...props}
      type="button"
      aria-labelledby={labelledby}
      class="flex h-control w-full items-center gap-2.5 rounded-md border border-line-strong bg-surface-2 px-3 text-left text-base text-fg transition-colors duration-[var(--dur-fast)] hover:bg-surface-3 focus-visible:border-focus"
    >
      <CurrentIcon size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
      <span class="shrink-0">{t(EVENT_LABELS[value])}</span>
      <span class="min-w-0 flex-1 truncate text-sm text-fg-3">{t(EVENT_HINTS[value])}</span>
      <ChevronDown size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div role="none" class="flex flex-col">
      {#each EVENTS as event (event)}
        {@const Icon = EVENT_ICONS[event]}

        <button
          type="button"
          role="menuitemradio"
          aria-checked={event === value}
          data-menu-item
          class={cx('flex items-center gap-2.5 rounded-sm px-2.5 py-2 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover')}
          onclick={() => {
            close();
            onchange(event);
          }}
        >
          <Icon size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
          <span class="flex min-w-0 flex-1 flex-col">
            <span class={cx('truncate text-sm', event === value ? 'text-fg' : 'text-fg-2')}>{t(EVENT_LABELS[event])}</span>
            <span class="truncate text-xs text-fg-3">{t(EVENT_HINTS[event])}</span>
          </span>
          {#if event === value}<Check size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />{/if}
        </button>
      {/each}
    </div>
  {/snippet}
</Popover>
