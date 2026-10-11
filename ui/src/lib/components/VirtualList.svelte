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
    /** True while the view sits at the end (within a few pixels). */
    atBottom?: boolean;
    class?: string;
    row: Snippet<[T, number]>;
  }

  let { items, estimate = 48, variable = false, overscan = 6, label, key, stick = false, atBottom = $bindable(true), class: className, row }: Props = $props();

  const BOTTOM_SLACK_PX = 24;

  let scroller: HTMLElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(0);
  let measured = $state.raw(new Map<string, number>());
  let following = $state(true);
  let lastTop = 0;

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

  /** Scrolling up leaves follow mode; coming back to the end re-enters it. */
  function onscroll(): void {
    if (!scroller) {
      return;
    }

    const top = scroller.scrollTop;
    const gap = scroller.scrollHeight - top - scroller.clientHeight;

    scrollTop = top;
    atBottom = gap < BOTTOM_SLACK_PX;

    if (atBottom) {
      following = true;
    } else if (top < lastTop - 1) {
      following = false;
    }

    lastTop = top;
  }

  // Keep the end in view as rows are added or grow, one write per frame.
  $effect(() => {
    void [offsets[items.length], viewport, stick, following];
    const element = scroller;
    if (!stick || !following || !element) {
      return;
    }

    const frame = requestAnimationFrame(() => (element.scrollTop = element.scrollHeight));

    return () => cancelAnimationFrame(frame);
  });

  export function scrollToIndex(index: number): void {
    if (scroller && index >= 0 && index < items.length) {
      scroller.scrollTop = offsets[index];
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

<div bind:this={scroller} bind:clientHeight={viewport} {onscroll} role="list" aria-label={label} class={cx('relative overflow-y-auto', className)}>
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
