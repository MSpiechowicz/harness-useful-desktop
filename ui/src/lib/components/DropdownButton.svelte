<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  import Button from './Button.svelte';
  import { ICON, type IconComponent } from './icon';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'class'> {
    variant?: 'primary' | 'secondary';
    icon: IconComponent;
    class?: string;
    children: Snippet;
  }

  let { variant = 'secondary', icon: Icon, children, ...rest }: Props = $props();
</script>

<!-- A button that opens a menu: an icon, the label, and a chevron. Spread the trigger props of a `Popover` onto it. -->
<Button {variant} {...rest}>
  {#snippet leading()}<Icon size={ICON.sm} aria-hidden="true" />{/snippet}
  {@render children()}
  {#snippet trailing()}<ChevronDown size={ICON.sm} aria-hidden="true" />{/snippet}
</Button>
