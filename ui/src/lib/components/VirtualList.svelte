<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';
  import { onDestroy } from 'svelte';

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
    key?: (item: T, index: number) => string | number;
    class?: string;
    row: Snippet<[T, number]>;
  }

  let { items, estimate = 48, variable = false, overscan = 6, label, key, class: className, row }: Props = $props();

  let scroller: HTMLElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(0);
  let measured = $state.raw<number[]>([]);

  const offsets = $derived(computeOffsets(items.length, (index) => (variable ? (measured[index] ?? estimate) : estimate)));
  const visible = $derived(computeWindow(offsets, scrollTop, viewport, overscan));
  const indexes = $derived(Array.from({ length: visible.end - visible.start }, (_, offset) => visible.start + offset));

  // One shared observer; rows report their index through `data-index`.
  const observer =
    typeof ResizeObserver === 'undefined'
      ? undefined
      : new ResizeObserver((entries) => {
          const next = [...measured];
          let changed = false;

          for (const entry of entries) {
            const index = Number((entry.target as HTMLElement).dataset.index);
            const height = Math.round(entry.borderBoxSize?.[0]?.blockSize ?? (entry.target as HTMLElement).offsetHeight);

            if (next[index] !== height) {
              next[index] = height;
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

  export function scrollToIndex(index: number): void {
    if (scroller && index >= 0 && index < items.length) {
      scroller.scrollTop = offsets[index];
    }
  }

  export function scrollToEnd(): void {
    if (scroller) {
      scroller.scrollTop = offsets[items.length];
    }
  }
</script>

<div
  bind:this={scroller}
  bind:clientHeight={viewport}
  onscroll={() => (scrollTop = scroller?.scrollTop ?? 0)}
  role="list"
  aria-label={label}
  class={cx('relative overflow-y-auto', className)}
>
  <div class="relative w-full" style:height="{visible.total}px">
    {#each indexes as index (key ? key(items[index], index) : index)}
      <div
        role="listitem"
        aria-posinset={index + 1}
        aria-setsize={items.length}
        data-index={index}
        use:track
        class="absolute inset-x-0 top-0"
        style:height={variable ? undefined : `${estimate}px`}
        style:transform="translateY({offsets[index]}px)"
      >
        {@render row(items[index], index)}
      </div>
    {/each}
  </div>
</div>
