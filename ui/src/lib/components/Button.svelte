<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  import { cx } from '$lib/utils/cx';

  import Spinner from './Spinner.svelte';

  type Variant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'dangerQuiet';
  type Size = 'sm' | 'md' | 'lg';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'class'> {
    class?: string;
    variant?: Variant;
    size?: Size;
    loading?: boolean;
    leading?: Snippet;
    trailing?: Snippet;
    children?: Snippet;
  }

  let {
    variant = 'secondary',
    size = 'md',
    loading = false,
    disabled = false,
    type = 'button',
    leading,
    trailing,
    children,
    class: className,
    ...rest
  }: Props = $props();

  // A translucent white veil that fades in on hover while keeping white text above 4.5:1.
  const VEIL =
    'text-on-accent shadow-float before:absolute before:inset-0 before:-z-10 before:bg-white/8 before:opacity-0 before:transition-opacity hover:before:opacity-100';

  const VARIANTS: Record<Variant, string> = {
    primary: `bg-[image:var(--brand-fill)] ${VEIL}`,
    secondary: 'border border-line-strong bg-surface-2 text-fg shadow-raised hover:bg-surface-3',
    ghost: 'text-fg-2 hover:bg-hover hover:text-fg',
    danger: `bg-danger-fill ${VEIL}`,
    // A destructive action that is not the main one: red text, no fill until it is hovered.
    dangerQuiet: 'border border-danger/30 text-danger hover:bg-danger-wash active:bg-danger-wash',
  };
  const SIZES: Record<Size, string> = {
    sm: 'h-8 gap-1.5 rounded-sm px-3 text-sm',
    md: 'h-9 gap-2 rounded-md px-4 text-base',
    lg: 'h-11 gap-2.5 rounded-md px-5 text-lg',
  };
</script>

<button
  {type}
  disabled={disabled || loading}
  aria-busy={loading || undefined}
  class={cx(
    'relative isolate inline-flex select-none items-center justify-center overflow-hidden font-medium whitespace-nowrap transition-[transform,background-color,color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-50 disabled:active:scale-100',
    VARIANTS[variant],
    SIZES[size],
    className,
  )}
  {...rest}
>
  {#if loading}
    <Spinner size={16} />
  {:else if leading}
    {@render leading()}
  {/if}

  {#if children}
    {@render children()}
  {/if}

  {#if trailing && !loading}
    {@render trailing()}
  {/if}
</button>
