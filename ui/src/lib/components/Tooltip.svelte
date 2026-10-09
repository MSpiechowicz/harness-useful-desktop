<script lang="ts">
  import type { Snippet } from 'svelte';

  import { rise } from '$lib/design/motion';
  import { uid } from '$lib/utils/ids';
  import { cx } from '$lib/utils/cx';

  type Side = 'top' | 'bottom' | 'right';

  interface Props {
    text: string;
    side?: Side;
    /** Receives the id to wire into the trigger's `aria-describedby`. */
    children: Snippet<[{ describedby: string }]>;
  }

  let { text, side = 'top', children }: Props = $props();

  const SHOW_DELAY_MS = 350;
  const PLACEMENT: Record<Side, string> = {
    top: 'bottom-full left-1/2 mb-2 -translate-x-1/2',
    bottom: 'top-full left-1/2 mt-2 -translate-x-1/2',
    right: 'left-full top-1/2 ml-3 -translate-y-1/2',
  };

  const id = uid('tooltip');
  let visible = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function showSoon(): void {
    clearTimeout(timer);
    timer = setTimeout(() => (visible = true), SHOW_DELAY_MS);
  }

  function showNow(): void {
    clearTimeout(timer);
    visible = true;
  }

  // Only keyboard focus reveals the tooltip; clicks and programmatic focus would just add noise.
  function onFocusIn(event: FocusEvent): void {
    if (event.target instanceof HTMLElement && event.target.matches(':focus-visible')) {
      showNow();
    }
  }

  function hide(): void {
    clearTimeout(timer);
    visible = false;
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      hide();
    }
  }
</script>

<span
  class="relative inline-flex"
  onpointerenter={showSoon}
  onpointerleave={hide}
  onfocusin={onFocusIn}
  onfocusout={hide}
  onkeydown={onKeydown}
  role="presentation"
>
  {@render children({ describedby: id })}

  {#if visible}
    <span
      {id}
      role="tooltip"
      transition:rise={{ y: 4, duration: 140 }}
      class={cx(
        'pointer-events-none absolute z-50 rounded-sm border border-line-strong bg-surface-4 px-2 py-1 text-xs font-medium whitespace-nowrap text-fg shadow-float',
        PLACEMENT[side],
      )}
    >
      {text}
    </span>
  {/if}
</span>
