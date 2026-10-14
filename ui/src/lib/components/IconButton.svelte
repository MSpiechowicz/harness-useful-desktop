<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';

  import { cx } from '$lib/utils/cx';

  import { ICON, type IconComponent } from './icon';
  import Tooltip from './Tooltip.svelte';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'aria-label' | 'class'> {
    class?: string;
    icon: IconComponent;
    /** Required: used as the accessible name and the tooltip text. */
    label: string;
    /** Shortcut combo shown in the tooltip, e.g. `mod+k`. */
    shortcut?: string;
    side?: 'top' | 'bottom' | 'right';
    /** Glyph size in px; the button keeps its own size. */
    iconSize?: number;
  }

  let { icon: Icon, label, shortcut, side = 'top', iconSize = ICON.md, type = 'button', class: className, ...rest }: Props = $props();
</script>

<Tooltip text={label} {shortcut} {side}>
  {#snippet children({ describedby })}
    <button
      {type}
      aria-label={label}
      aria-describedby={describedby}
      class={cx(
        'inline-grid place-items-center rounded-md text-fg-2 transition-[transform,background-color,color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] hover:bg-hover hover:text-fg active:scale-90 disabled:cursor-not-allowed disabled:opacity-50',
        'size-control',
        className,
      )}
      {...rest}
    >
      <Icon size={iconSize} strokeWidth={1.8} aria-hidden="true" />
    </button>
  {/snippet}
</Tooltip>
