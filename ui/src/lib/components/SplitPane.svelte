<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  interface Props {
    /** `horizontal` places panes side by side; `vertical` stacks them. */
    direction?: 'horizontal' | 'vertical';
    /** Size in px of the first pane. */
    size?: number;
    min?: number;
    /** Minimum size in px of the second pane. */
    minSecond?: number;
    label: string;
    first: Snippet;
    second: Snippet;
  }

  let { direction = 'horizontal', size = $bindable(280), min = 160, minSecond = 160, label, first, second }: Props = $props();

  const KEY_STEP = 16;
  const KEY_STEP_LARGE = 64;
  const initialSize = size;

  const horizontal = $derived(direction === 'horizontal');

  let container: HTMLElement | undefined = $state();
  let dragging = $state(false);
  let dragOrigin = { pointer: 0, size: 0 };

  function available(): number {
    if (!container) {
      return Number.POSITIVE_INFINITY;
    }

    return (horizontal ? container.clientWidth : container.clientHeight) - minSecond;
  }

  function clamp(value: number): number {
    return Math.round(Math.min(Math.max(value, min), Math.max(min, available())));
  }

  function pointerPosition(event: PointerEvent): number {
    return horizontal ? event.clientX : event.clientY;
  }

  function onPointerDown(event: PointerEvent): void {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragOrigin = { pointer: pointerPosition(event), size };
    dragging = true;
  }

  function onPointerMove(event: PointerEvent): void {
    if (dragging) {
      size = clamp(dragOrigin.size + pointerPosition(event) - dragOrigin.pointer);
    }
  }

  function onPointerUp(): void {
    dragging = false;
  }

  function onKeydown(event: KeyboardEvent): void {
    const step = event.shiftKey ? KEY_STEP_LARGE : KEY_STEP;
    const grow = horizontal ? 'ArrowRight' : 'ArrowDown';
    const shrink = horizontal ? 'ArrowLeft' : 'ArrowUp';

    if (event.key === grow) {
      size = clamp(size + step);
    } else if (event.key === shrink) {
      size = clamp(size - step);
    } else if (event.key === 'Home') {
      size = clamp(min);
    } else if (event.key === 'End') {
      size = clamp(available());
    } else {
      return;
    }

    event.preventDefault();
  }
</script>

<div bind:this={container} class={cx('flex size-full min-h-0 min-w-0', horizontal ? 'flex-row' : 'flex-col')}>
  <div class="min-h-0 min-w-0 overflow-hidden" style:flex="0 0 {size}px">
    {@render first()}
  </div>

  <!-- The WAI-ARIA window splitter is a focusable separator; Svelte's lint does not model that. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    role="separator"
    tabindex="0"
    aria-label={label}
    aria-orientation={horizontal ? 'vertical' : 'horizontal'}
    aria-valuenow={size}
    aria-valuemin={min}
    aria-valuemax={Math.round(available())}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    onkeydown={onKeydown}
    ondblclick={() => (size = clamp(initialSize))}
    class={cx(
      'group relative z-10 shrink-0 touch-none',
      horizontal ? 'w-1 cursor-col-resize' : 'h-1 cursor-row-resize',
    )}
  >
    <span
      aria-hidden="true"
      class={cx(
        'absolute transition-colors duration-[var(--dur-fast)]',
        horizontal ? 'inset-y-0 left-1/2 w-px -translate-x-1/2' : 'inset-x-0 top-1/2 h-px -translate-y-1/2',
        dragging ? 'bg-accent' : 'bg-line-strong group-hover:bg-accent',
      )}
    ></span>
  </div>

  <div class="min-h-0 min-w-0 flex-1 overflow-hidden">
    {@render second()}
  </div>
</div>
