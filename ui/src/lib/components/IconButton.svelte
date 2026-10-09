<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';

  import { cx } from '$lib/utils/cx';

  import { ICON, type IconComponent } from './icon';
  import Tooltip from './Tooltip.svelte';

  type Size = 'sm' | 'md' | 'lg';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'aria-label' | 'class'> {
    class?: string;
    icon: IconComponent;
    /** Required: used as the accessible name and the tooltip text. */
    label: string;
    /** Shortcut combo shown in the tooltip, e.g. `mod+k`. */
    shortcut?: string;
    size?: Size;
    side?: 'top' | 'bottom' | 'right';
  }

  let { icon: Icon, label, shortcut, size = 'md', side = 'top', type = 'button', class: className, ...rest }: Props = $props();

  const BOX: Record<Size, string> = { sm: 'size-7', md: 'size-9', lg: 'size-11' };
  const GLYPH: Record<Size, number> = { sm: ICON.sm, md: ICON.md, lg: ICON.lg };
</script>

<Tooltip text={label} {shortcut} {side}>
  {#snippet children({ describedby })}
    <button
      {type}
      aria-label={label}
      aria-describedby={describedby}
      class={cx(
        'inline-grid place-items-center rounded-md text-fg-2 transition-[transform,background-color,color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] hover:bg-hover hover:text-fg active:scale-90 disabled:cursor-not-allowed disabled:opacity-50',
        BOX[size],
        className,
      )}
      {...rest}
    >
      <Icon size={GLYPH[size]} strokeWidth={1.8} aria-hidden="true" />
    </button>
  {/snippet}
</Tooltip>
