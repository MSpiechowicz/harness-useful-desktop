<script lang="ts">
  import type { Snippet } from 'svelte';

  import { DURATION, scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** `horizontal` places panes side by side; `vertical` stacks them. */
    direction?: 'horizontal' | 'vertical';
    /** Size in px the user wants for the first pane. The pane shows it, or less while the second pane would be left under `minSecond`. */
    size?: number;
    /** Where a double click on the divider puts the first pane back; defaults to the size it started at. */
    resetTo?: number;
    /** Tucks the first pane away (its width animates to nothing, the divider goes) without unmounting it. */
    collapsed?: boolean;
    min?: number;
    /** Minimum size in px of the second pane. */
    minSecond?: number;
    label: string;
    /** Receives the size in px the first pane shows now. */
    first: Snippet<[number]>;
    second: Snippet;
  }

  let { direction = 'horizontal', size = $bindable(280), resetTo, collapsed = false, min = 160, minSecond = 160, label, first, second }: Props = $props();

  const KEY_STEP = 16;
  const KEY_STEP_LARGE = 64;
  const initialSize = size;

  const horizontal = $derived(direction === 'horizontal');

  let container: HTMLElement | undefined = $state();
  // Measured, so growing the second pane's minimum or shrinking the window is noticed (0 until laid out).
  let containerWidth = $state(0);
  let containerHeight = $state(0);
  let dragging = $state(false);
  let animating = $state(false);
  let dragOrigin = { pointer: 0, size: 0 };

  const extent = $derived(horizontal ? containerWidth : containerHeight);

  /** The most the first pane can have while the second keeps `minSecond`; unlimited until the pane is measured. */
  const available = $derived(extent > 0 ? extent - minSecond : Number.POSITIVE_INFINITY);

  function clamp(value: number): number {
    return Math.round(Math.min(Math.max(value, min), Math.max(min, available)));
  }

  // `size` is what the user chose and is never rewritten to fit: a window that shrinks, or a second pane that asks for more
  // room (a details column appears), only shows the first pane smaller, and it comes back when the room does.
  const shown = $derived(clamp(size));

  // Only a pane that is tucked away or brought back moves slowly. A fit to the window, a drag or a key press follows at once.
  let firstRun = true;

  // Before the update, so the transition is on when the size starts to change.
  $effect.pre(() => {
    void collapsed;

    if (firstRun) {
      firstRun = false;

      return;
    }

    animating = true;
    const timer = setTimeout(() => (animating = false), scaled(DURATION.base));

    return () => clearTimeout(timer);
  });

  function pointerPosition(event: PointerEvent): number {
    return horizontal ? event.clientX : event.clientY;
  }

  function onPointerDown(event: PointerEvent): void {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragOrigin = { pointer: pointerPosition(event), size: shown };
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
      size = clamp(shown + step);
    } else if (event.key === shrink) {
      size = clamp(shown - step);
    } else if (event.key === 'Home') {
      size = clamp(min);
    } else if (event.key === 'End') {
      size = clamp(available);
    } else {
      return;
    }

    event.preventDefault();
  }
</script>

<div bind:this={container} bind:clientWidth={containerWidth} bind:clientHeight={containerHeight} class={cx('flex size-full min-h-0 min-w-0', horizontal ? 'flex-row' : 'flex-col')}>
  <div
    inert={collapsed}
    aria-hidden={collapsed ? true : undefined}
    class={cx(
      'min-h-0 min-w-0 overflow-hidden',
      animating && !dragging && 'transition-[flex-basis,opacity] duration-[var(--dur-base)] ease-[var(--ease-out-expo)]',
      collapsed && 'opacity-0',
    )}
    style:flex="0 0 {collapsed ? 0 : shown}px"
  >
    {@render first(shown)}
  </div>

  <!-- The WAI-ARIA window splitter is a focusable separator; Svelte's lint does not model that. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    role="separator"
    tabindex={collapsed ? -1 : 0}
    inert={collapsed}
    aria-hidden={collapsed ? true : undefined}
    aria-label={label}
    aria-orientation={horizontal ? 'vertical' : 'horizontal'}
    aria-valuenow={shown}
    aria-valuemin={min}
    aria-valuemax={Math.round(Math.max(min, available))}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    onkeydown={onKeydown}
    ondblclick={() => (size = clamp(resetTo ?? initialSize))}
    class={cx(
      'group relative z-10 shrink-0 touch-none',
      horizontal ? cx('cursor-col-resize', collapsed ? 'w-0' : 'w-1') : cx('cursor-row-resize', collapsed ? 'h-0' : 'h-1'),
    )}
  >
    <span
      aria-hidden="true"
      class={cx(
        'absolute transition-colors duration-[var(--dur-fast)]',
        horizontal ? 'inset-y-0 left-0 w-px' : 'inset-x-0 top-0 h-px',
        dragging ? 'bg-accent' : 'bg-panel-edge group-hover:bg-accent',
        collapsed && 'hidden',
      )}
    ></span>
  </div>

  <div class="min-h-0 min-w-0 flex-1 overflow-hidden">
    {@render second()}
  </div>
</div>
