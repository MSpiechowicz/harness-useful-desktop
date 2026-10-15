<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  import { cx } from '$lib/utils/cx';

  import { ICON, type IconComponent } from './icon';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'class'> {
    /** `danger` for an action that destroys something. */
    tone?: 'default' | 'danger';
    /** Shown before the label. */
    icon?: IconComponent;
    class?: string;
    children: Snippet;
  }

  let { tone = 'default', icon: Icon, class: className, children, ...rest }: Props = $props();
</script>

<!-- One row of a menu opened by a `Popover` of kind "menu": it takes part in its arrow-key navigation. -->
<button
  type="button"
  role="menuitem"
  data-menu-item
  {...rest}
  class={cx(
    'flex items-center gap-2 rounded-sm focus-visible:outline-offset-[-2px] px-2.5 py-2 text-left text-sm transition-colors duration-[var(--dur-fast)] disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:bg-transparent',
    tone === 'danger' ? 'text-danger hover:bg-danger-wash focus-visible:bg-danger-wash' : 'text-fg hover:bg-hover focus-visible:bg-hover',
    className,
  )}
>
  {#if Icon}<Icon size={ICON.sm} aria-hidden="true" />{/if}
  {@render children()}
</button>
