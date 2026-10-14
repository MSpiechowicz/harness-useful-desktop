<script lang="ts">
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
    /** Leave room at the right edge for a control laid over the row. */
    reserve?: boolean;
    button?: HTMLButtonElement;
    onselect: () => void;
  }

  let { view, shortcut, active, expanded, badge, reserve = false, button = $bindable(), onselect }: Props = $props();

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
        // Pixel sizes, not rem: this app's root font is 14px, the dashboard's 16px, and the rows match the dashboard's.
        'group/item relative flex h-[28px] w-full items-center gap-[10px] rounded-sm text-left transition-[color,background-color,padding] duration-[var(--dur-fast)]',
        // Expanded the dashboard's 12px padding; collapsed the icon centers: (64px rail - 1px border - 16px nav padding - 16px icon) / 2.
        expanded ? 'px-[12px]' : 'px-[15.5px]',
        expanded && reserve && 'pr-[34px]',
        active ? 'font-medium text-accent-text' : 'text-fg-2 hover:bg-hover hover:text-fg',
      )}
    >
      <span class="relative grid size-[16px] shrink-0 place-items-center">
        <view.icon size={ICON.sm} aria-hidden="true" />

        {#if badge && !expanded}
          <span
            aria-hidden="true"
            class={cx(
              'absolute -top-[6px] -right-[8px] grid h-[16px] min-w-[16px] place-items-center rounded-full border border-surface-1 px-1 text-[10px] leading-none font-semibold',
              badge.tone === 'warning' ? 'bg-warning text-surface-1' : 'bg-accent-fill text-on-accent',
              badge.text === '' && 'size-2.5 min-w-0 p-0',
              badge.busy && 'animate-pulse',
            )}
          >{badge.text}</span>
        {/if}
      </span>

      <span class={cx('min-w-0 flex-1 truncate text-base whitespace-nowrap transition-opacity duration-[var(--dur-base)]', expanded ? 'opacity-100' : 'opacity-0')} aria-hidden={!expanded}>
        {view.label}
      </span>

      {#if expanded}
        {#if badge && badge.text !== ''}
          <span aria-hidden="true" class={cx('tabular rounded-full px-1.5 text-xs leading-5', badge.tone === 'warning' ? 'bg-warning-wash text-warning' : 'bg-surface-3 text-fg-2')}>{badge.text}</span>
        {:else if badge}
          <span aria-hidden="true" class={cx('size-2 rounded-full', badge.tone === 'warning' ? 'bg-warning' : 'bg-accent', badge.busy && 'animate-pulse')}></span>
        {/if}

      {/if}
    </button>
  {/snippet}
</Tooltip>
