<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The name of the column, for the accessible label. */
    column: string;
    /** The edge of its header cell the handle is on. A drag, or an arrow key, moves that edge: the column grows when the edge moves away from it. */
    edge: 'start' | 'end';
    /** The width in px the column has now. */
    measure: () => number;
    /** Asks for a width; answers the width it got, which is held between the least and the most the table allows. */
    onresize: (px: number) => number;
    /** The drag or the key press is over: the width is kept. */
    oncommit: () => void;
    /** Back to the width the column started with. */
    onreset: () => void;
  }

  let { column, edge, measure, onresize, oncommit, onreset }: Props = $props();

  const KEY_STEP = 16;

  /** Which way the column grows when the edge moves right. */
  const direction = $derived(edge === 'end' ? 1 : -1);

  let dragging = $state(false);
  let width = $state(0);
  let origin = { pointer: 0, width: 0 };

  /** While a drag lasts, nothing on the page is selected by it and the pointer keeps its shape wherever it strays. */
  function hold(on: boolean): void {
    const root = document.documentElement.style;

    root.userSelect = on ? 'none' : '';
    root.cursor = on ? 'col-resize' : '';
  }

  function onPointerDown(event: PointerEvent): void {
    // Without this the drag starts a text selection over the headers it passes.
    event.preventDefault();
    window.getSelection()?.removeAllRanges();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    hold(true);
    width = measure();
    origin = { pointer: event.clientX, width };
    dragging = true;
  }

  function onPointerMove(event: PointerEvent): void {
    if (dragging) {
      width = onresize(origin.width + direction * (event.clientX - origin.pointer));
    }
  }

  function onPointerUp(): void {
    if (dragging) {
      dragging = false;
      hold(false);
      oncommit();
    }
  }

  // A handle that goes away mid-drag (the table was replaced) must not leave the page unselectable.
  $effect(() => () => hold(false));

  function onKeydown(event: KeyboardEvent): void {
    const moved = event.key === 'ArrowRight' ? KEY_STEP : event.key === 'ArrowLeft' ? -KEY_STEP : 0;

    if (moved === 0) {
      return;
    }

    event.preventDefault();
    width = onresize(measure() + direction * moved);
    oncommit();
  }
</script>

<!-- A short line on the edge of a header cell, in the line colour at rest and in the accent under the pointer, on focus and while it
     is dragged. The line is small, and the hit area around it is as tall as the header and 8px wide, so it stays easy to grab.
     It is a window splitter, so it is a focusable separator as in `SplitPane`; Svelte's lint does not model that. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  role="separator"
  tabindex="0"
  aria-orientation="vertical"
  aria-label={t('components.table.resize', { column })}
  aria-valuenow={width}
  onfocus={() => (width = measure())}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
  onkeydown={onKeydown}
  ondblclick={onreset}
  class={cx('group/handle absolute inset-y-0 z-10 flex w-2 touch-none cursor-col-resize items-center justify-center focus-visible:outline-none', edge === 'end' ? '-right-1' : '-left-1')}
>
  <span
    aria-hidden="true"
    class={cx(
      'h-3.5 w-px rounded-full transition-colors duration-[var(--dur-fast)] group-hover/handle:bg-accent group-focus-visible/handle:bg-accent',
      dragging ? 'bg-accent' : 'bg-line',
    )}
  ></span>
</div>
