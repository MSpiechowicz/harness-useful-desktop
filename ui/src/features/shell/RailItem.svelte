<script lang="ts">
  import Kbd from '$lib/components/Kbd.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { ICON } from '$lib/components/icon';
  import { cx } from '$lib/utils/cx';

  import type { RailBadge } from './rail-badges';
  import type { ViewMeta } from './views';

  interface Props {
    view: ViewMeta;
    shortcut: string;
    active: boolean;
    /** Show the label next to the icon. */
    expanded: boolean;
    badge: RailBadge | null;
    button?: HTMLButtonElement;
    onselect: () => void;
  }

  let { view, shortcut, active, expanded, badge, button = $bindable(), onselect }: Props = $props();

  const description = $derived(badge ? `${view.label}, ${badge.label}` : view.label);
</script>

<Tooltip text={view.label} {shortcut} side="right" disabled={expanded} class="flex w-full">
  {#snippet children({ describedby })}
    <button
      bind:this={button}
      type="button"
      aria-label={description}
      aria-current={active ? 'page' : undefined}
      aria-describedby={describedby}
      onclick={onselect}
      class={cx(
        'group/item relative flex h-10 w-full items-center gap-3 rounded-md px-[9.5px] text-left transition-[color,background-color] duration-[var(--dur-fast)]',
        active ? 'text-accent-text' : 'text-fg-2 hover:bg-hover hover:text-fg',
      )}
    >
      <!-- Sized in pixels like the icon, so the collapsed rail centers it: (64px rail - 1px border - 24px padding - 20px icon) / 2. -->
      <span class="relative grid size-[20px] shrink-0 place-items-center">
        <view.icon size={ICON.lg} strokeWidth={1.8} aria-hidden="true" />

        {#if badge && !expanded}
          <span
            aria-hidden="true"
            class={cx(
              'absolute -top-1.5 -right-2 grid h-4 min-w-4 place-items-center rounded-full border border-surface-1 px-1 text-[10px] leading-none font-semibold',
              badge.tone === 'warning' ? 'bg-warning text-surface-1' : 'bg-accent-fill text-on-accent',
              badge.text === '' && 'size-2.5 min-w-0 p-0',
              badge.busy && 'animate-pulse',
            )}
          >{badge.text}</span>
        {/if}
      </span>

      <span class={cx('min-w-0 flex-1 truncate text-base font-medium whitespace-nowrap transition-opacity duration-[var(--dur-base)]', expanded ? 'opacity-100' : 'opacity-0')} aria-hidden={!expanded}>
        {view.label}
      </span>

      {#if expanded}
        {#if badge && badge.text !== ''}
          <span aria-hidden="true" class={cx('tabular rounded-full px-1.5 text-xs leading-5 group-hover/item:hidden group-focus-visible/item:hidden', badge.tone === 'warning' ? 'bg-warning-wash text-warning' : 'bg-surface-3 text-fg-2')}>{badge.text}</span>
        {:else if badge}
          <span aria-hidden="true" class={cx('size-2 rounded-full group-hover/item:hidden', badge.tone === 'warning' ? 'bg-warning' : 'bg-accent', badge.busy && 'animate-pulse')}></span>
        {/if}

        <span class="hidden shrink-0 group-hover/item:inline-flex group-focus-visible/item:inline-flex"><Kbd keys={shortcut} /></span>
      {/if}
    </button>
  {/snippet}
</Tooltip>
