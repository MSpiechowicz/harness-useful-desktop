<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import { portal } from '$lib/a11y/portal';
  import { pop, scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  interface Props {
    open: boolean;
    /** `top` anchors near the upper third (command palette); `center` centers (dialogs). */
    placement?: 'center' | 'top';
    /** Accessible name; use `labelledby` instead when a visible heading exists. */
    label?: string;
    labelledby?: string;
    describedby?: string;
    /** Whether Escape and scrim clicks close the overlay. */
    dismissible?: boolean;
    onclose: () => void;
    class?: string;
    children: Snippet;
  }

  let {
    open,
    placement = 'center',
    label,
    labelledby,
    describedby,
    dismissible = true,
    onclose,
    class: className,
    children,
  }: Props = $props();

  function dismiss(): void {
    if (dismissible) {
      onclose();
    }
  }
</script>

{#if open}
  <div use:portal class="fixed inset-0 z-[80]">
    <button
      type="button"
      tabindex="-1"
      aria-hidden="true"
      class="absolute inset-0 cursor-default bg-scrim backdrop-blur-sm"
      transition:fade={{ duration: scaled(200) }}
      onclick={dismiss}
    ></button>

    <div class={cx('pointer-events-none absolute inset-0 flex justify-center p-4', placement === 'top' ? 'items-start pt-[14vh]' : 'items-center')}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={label}
        aria-labelledby={labelledby}
        aria-describedby={describedby}
        use:focusTrap={{ onEscape: dismissible ? onclose : undefined }}
        in:pop={{ duration: 360 }}
        out:pop={{ duration: 160, spring: false }}
        class={cx('pointer-events-auto max-h-full max-w-full outline-none', className)}
      >
        {@render children()}
      </div>
    </div>
  </div>
{/if}
