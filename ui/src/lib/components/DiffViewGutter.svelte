<script lang="ts">
  import type { PickHandlers } from '$lib/git/comment-range';
  import type { CommentSide } from '$lib/ipc/bindings/CommentSide';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The line number on this side, or null when the line has none here. */
    number: number | null;
    side: CommentSide;
    /** Flat index of the line in the file's diff. */
    index: number;
    pick: PickHandlers;
    class?: string;
  }

  let { number, side, index, pick, class: className }: Props = $props();

  const BASE = 'w-12 shrink-0 select-none px-1.5 text-right font-mono text-xs leading-[18px] tabular text-fg-3';

  function onpointerdown(event: PointerEvent): void {
    if (event.button === 0) {
      event.preventDefault();
      pick.start(index, side, event.shiftKey);
    }
  }

  // Keyboard activation arrives as a click without a pointer (detail 0); mouse clicks are handled on pointer down.
  function onclick(event: MouseEvent): void {
    if (event.detail === 0) {
      pick.start(index, side, event.shiftKey);
      pick.finish();
    }
  }
</script>

{#if number === null}
  <span class={cx(BASE, className)} aria-hidden="true"></span>
{:else}
  <button
    type="button"
    aria-label="Comment on {side} line {number}"
    class={cx(BASE, 'cursor-pointer transition-colors duration-[var(--dur-fast)] hover:bg-accent-wash hover:text-accent-text focus-visible:text-accent-text', className)}
    {onpointerdown}
    onpointerenter={() => pick.extend(index, side)}
    {onclick}
  >
    {number}
  </button>
{/if}
