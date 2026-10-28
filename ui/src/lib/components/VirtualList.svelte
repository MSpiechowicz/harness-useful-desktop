<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';
  import { onDestroy, tick } from 'svelte';

  import { cx } from '$lib/utils/cx';
  import { computeOffsets, computeWindow } from '$lib/utils/virtual-window';

  interface Props {
    items: T[];
    /** Row height in px; the exact height when rows are fixed, the first guess when `variable`. */
    estimate?: number;
    /** Measure rows after render (for content of unknown height). */
    variable?: boolean;
    overscan?: number;
    label: string;
    /** Identity of a row. Measured heights follow it, so rows added at the front keep theirs. */
    key?: (item: T, index: number) => string | number;
    /** Follow the end while content grows, until the user scrolls up. */
    stick?: boolean;
    /** True while the view sits at the end (within a few pixels) or follows it. */
    atBottom?: boolean;
    class?: string;
    row: Snippet<[T, number]>;
  }

  let { items, estimate = 48, variable = false, overscan = 6, label, key, stick = false, atBottom = $bindable(true), class: className, row }: Props = $props();

  const BOTTOM_SLACK_PX = 24;
  /** Within this of the end counts as reached: a person scrolling down lands exactly there. */
  const END_PX = 2;
  /** A wheel turn, key press or swipe up counts as the cause of a scroll up for this long. */
  const INTENT_MS = 300;
  /** Keys that move a scrolled view up. */
  const UP_KEYS: Record<string, true> = { ArrowUp: true, PageUp: true, Home: true };

  let scroller: HTMLElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(0);
  let measured = $state.raw(new Map<string, number>());
  let following = $state(true);
  let lastTop = 0;
  /** Until when a scroll up is the person's doing (wheel, keys, swipe); a drag on the scrollbar lasts as long as it does. */
  let intentUntil = 0;
  let dragging = false;
  let touchY: number | null = null;

  function keyOf(index: number): string {
    return String(key ? key(items[index], index) : index);
  }

  const offsets = $derived(computeOffsets(items.length, (index) => (variable ? (measured.get(keyOf(index)) ?? estimate) : estimate)));
  const visible = $derived(computeWindow(offsets, scrollTop, viewport, overscan));
  const indexes = $derived(Array.from({ length: visible.end - visible.start }, (_, offset) => visible.start + offset));

  // One shared observer; rows report their identity through `data-key`.
  const observer =
    typeof ResizeObserver === 'undefined'
      ? undefined
      : new ResizeObserver((entries) => {
          const next = new Map(measured);
          let changed = false;

          for (const entry of entries) {
            const element = entry.target as HTMLElement;
            const height = Math.round(entry.borderBoxSize?.[0]?.blockSize ?? element.offsetHeight);

            if (next.get(element.dataset.key ?? '') !== height) {
              next.set(element.dataset.key ?? '', height);
              changed = true;
            }
          }

          if (changed) {
            measured = next;
          }
        });

  onDestroy(() => observer?.disconnect());

  function track(node: HTMLElement) {
    if (variable) {
      observer?.observe(node);
    }

    return {
      destroy() {
        observer?.unobserve(node);
      },
    };
  }

  /** Whether a scroll up that happens now is the person's: they just asked for it, or hold the scrollbar. */
  function wantsUp(): boolean {
    return dragging || performance.now() < intentUntil;
  }

  function intendUp(): void {
    intentUntil = performance.now() + INTENT_MS;
  }

  function syncBottom(): void {
    if (scroller) {
      const distance = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;

      atBottom = distance < BOTTOM_SLACK_PX || (stick && following);
    }
  }

  /**
   * Reaching the end re-enters follow mode. Leaving it takes a scroll up the person asked for (wheel, keys, swipe,
   * scrollbar): a falling `scrollTop` alone is no sign, since shrinking rows, clamping and the engine's own scroll
   * animations move it too, and treating those as the user would drop out of follow mid-stream. The request only
   * counts once the list really moves up, so a wheel turn inside a nested scroller or a zoom changes nothing.
   */
  function onscroll(): void {
    if (!scroller) {
      return;
    }

    const top = scroller.scrollTop;
    const movedUp = top < lastTop - 0.5;
    const distance = scroller.scrollHeight - top - scroller.clientHeight;

    lastTop = top;
    scrollTop = top;

    if (movedUp && wantsUp()) {
      following = false;
    } else if (distance <= END_PX) {
      following = true;
    }

    syncBottom();
  }

  // The requests are noted on the way past (passive, on the list or the window) and judged by `onscroll`.
  $effect(() => {
    const element = scroller;
    if (!element) {
      return;
    }

    const onwheel = (event: WheelEvent): void => {
      if (event.deltaY < 0 && !event.ctrlKey) {
        intendUp();
      }
    };

    const ontouchstart = (event: TouchEvent): void => {
      touchY = event.touches[0]?.clientY ?? null;
    };

    const ontouchmove = (event: TouchEvent): void => {
      const y = event.touches[0]?.clientY ?? null;
      if (touchY !== null && y !== null && y > touchY) {
        intendUp();
      }

      touchY = y;
    };

    const endTouch = (): void => {
      touchY = null;
    };

    // A press on the list element itself (not on a row) is on its scrollbar.
    const onpointerdown = (event: PointerEvent): void => {
      dragging = event.target === element;
    };

    const endDrag = (): void => {
      dragging = false;
    };

    // Keys scroll the list the person last clicked in, even when focus is on the page itself, so they are watched
    // on the window; typing in a field is not scrolling.
    const onkeydown = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement;
      const typing = target.isContentEditable || target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement;

      if (!typing && (UP_KEYS[event.key] || (event.key === ' ' && event.shiftKey)) && element.offsetParent !== null) {
        intendUp();
      }
    };

    element.addEventListener('wheel', onwheel, { passive: true });
    element.addEventListener('touchstart', ontouchstart, { passive: true });
    element.addEventListener('touchmove', ontouchmove, { passive: true });
    element.addEventListener('touchend', endTouch);
    element.addEventListener('touchcancel', endTouch);
    element.addEventListener('pointerdown', onpointerdown);
    window.addEventListener('pointerup', endDrag);
    window.addEventListener('pointercancel', endDrag);
    window.addEventListener('keydown', onkeydown);

    return () => {
      element.removeEventListener('wheel', onwheel);
      element.removeEventListener('touchstart', ontouchstart);
      element.removeEventListener('touchmove', ontouchmove);
      element.removeEventListener('touchend', endTouch);
      element.removeEventListener('touchcancel', endTouch);
      element.removeEventListener('pointerdown', onpointerdown);
      window.removeEventListener('pointerup', endDrag);
      window.removeEventListener('pointercancel', endDrag);
      window.removeEventListener('keydown', onkeydown);
    };
  });

  // Keep the end in view as rows are added or grow, one write per frame; `atBottom` follows the content too, not only the scrolling.
  $effect(() => {
    void [offsets[items.length], viewport, stick, following];
    const element = scroller;
    if (!element) {
      return;
    }

    const frame = requestAnimationFrame(() => {
      if (stick && following) {
        element.scrollTop = element.scrollHeight;
      }

      syncBottom();
    });

    return () => cancelAnimationFrame(frame);
  });

  /** Brings a row to the top of the view, or with `center` to the middle of it. */
  export function scrollToIndex(index: number, align: 'start' | 'center' = 'start'): void {
    if (scroller && index >= 0 && index < items.length) {
      // A jump is the caller's choice of place: the view stops following the end unless it lands there.
      following = false;
      scroller.scrollTop = align === 'center' ? Math.max(0, offsets[index] - (viewport - (offsets[index + 1] - offsets[index])) / 2) : offsets[index];
    }
  }

  export function scrollToEnd(): void {
    following = true;

    if (scroller) {
      scroller.scrollTop = scroller.scrollHeight;
    }
  }

  /** Runs `change` (which adds rows at the front) and keeps what the user was looking at in place. */
  export async function keepPosition(change: () => Promise<void>): Promise<void> {
    const before = scroller?.scrollHeight ?? 0;
    await change();
    await tick();

    if (scroller && scroller.scrollTop > 0) {
      scroller.scrollTop += scroller.scrollHeight - before;
    }
  }
</script>

<div
  bind:this={scroller}
  bind:clientHeight={viewport}
  {onscroll}
  role="list"
  aria-label={label}
  class={cx('relative overflow-y-auto', className)}
>
  <div class="relative w-full" style:height="{visible.total}px">
    {#each indexes as index (key ? key(items[index], index) : index)}
      <!-- `top`, not a transform: WebKitGTK composites transformed rows and blurs their text at fractional scales. -->
      <div
        role="listitem"
        aria-posinset={index + 1}
        aria-setsize={items.length}
        data-key={keyOf(index)}
        use:track
        class="absolute inset-x-0"
        style:height={variable ? undefined : `${estimate}px`}
        style:top="{offsets[index]}px"
      >
        {@render row(items[index], index)}
      </div>
    {/each}
  </div>
</div>
