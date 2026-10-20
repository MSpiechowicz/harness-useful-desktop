<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  import { cx } from '$lib/utils/cx';

  import Spinner from './Spinner.svelte';

  type Variant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'dangerQuiet';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'class'> {
    class?: string;
    variant?: Variant;
    /** The children are not a plain text label (an icon-only button, a truncating span): skip the label trim wrapper. */
    bare?: boolean;
    loading?: boolean;
    leading?: Snippet;
    trailing?: Snippet;
    children?: Snippet;
  }

  let {
    variant = 'secondary',
    bare = false,
    loading = false,
    disabled = false,
    type = 'button',
    leading,
    trailing,
    children,
    class: className,
    ...rest
  }: Props = $props();

  // A translucent white veil that fades in on hover while keeping white text above 4.5:1. It fades its colour, not its
  // opacity: an opacity animation gives the button its own compositing layer in WebKitGTK, which snaps the label by
  // half a pixel on hover-in and hover-out.
  const VEIL =
    'text-on-accent shadow-float before:absolute before:inset-0 before:-z-10 before:bg-white/0 before:transition-colors hover:before:bg-white/8';

  const VARIANTS: Record<Variant, string> = {
    primary: `bg-[image:var(--brand-fill)] ${VEIL}`,
    secondary: 'border border-line-strong bg-surface-2 text-fg shadow-raised hover:bg-surface-3',
    ghost: 'text-fg-2 hover:bg-hover hover:text-fg',
    danger: `bg-danger-fill ${VEIL}`,
    // A destructive action that is not the main one: red text, no fill until it is hovered.
    dangerQuiet: 'border border-danger/30 text-danger hover:bg-danger-wash active:bg-danger-wash',
  };
  const SIZE = 'h-control gap-2 rounded-md px-4 text-base';
  // An icon's glyph sits inside its box, so the side it is on gets one step less padding to look as tight as the text side.
  const ICON_SIDE = { leading: 'pl-3', trailing: 'pr-3' } as const;
</script>

<button
  {type}
  disabled={disabled || loading}
  aria-busy={loading || undefined}
  class={cx(
    'relative isolate inline-flex select-none items-center justify-center overflow-hidden font-medium whitespace-nowrap transition-[transform,background-color,color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-50 disabled:active:scale-100',
    VARIANTS[variant],
    SIZE,
    (leading || loading) && children && ICON_SIDE.leading,
    trailing && children && ICON_SIDE.trailing,
    className,
  )}
  {...rest}
>
  {#if loading}
    <Spinner size={16} />
  {:else if leading}
    {@render leading()}
  {/if}

  {#if children && bare}
    {@render children()}
  {:else if children}
    <!-- Trimmed to the x-height and the baseline, so the label's lowercase body (not the line box with its leading) sits on the button's centre. -->
    <span class="label-trim">{@render children()}</span>
  {/if}

  {#if trailing && !loading}
    {@render trailing()}
  {/if}
</button>
