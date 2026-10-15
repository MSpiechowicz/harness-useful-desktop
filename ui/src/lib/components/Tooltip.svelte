<script lang="ts">
  import type { Snippet } from 'svelte';

  import { portal } from '$lib/a11y/portal';
  import { rise } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';
  import { floatingPosition } from '$lib/utils/floating';
  import { uid } from '$lib/utils/ids';

  import Kbd from './Kbd.svelte';

  type Side = 'top' | 'bottom' | 'right';

  interface Props {
    text?: string;
    /** Roomy free-form content (a list of points) instead of `text`; the tooltip becomes a small panel. */
    rich?: Snippet;
    /** A second, quieter paragraph under the text; line breaks are kept and long lines wrap. */
    detail?: string;
    /** Shortcut combo such as `mod+k`, shown as key caps next to the text. */
    shortcut?: string;
    side?: Side;
    /** Suppresses the tooltip (e.g. when the hint only applies in some states). */
    disabled?: boolean;
    /** Layout classes of the wrapper; defaults to `inline-flex`. */
    class?: string;
    /** Receives the id to wire into the trigger's `aria-describedby`. */
    children: Snippet<[{ describedby: string }]>;
  }

  let { text = '', rich, detail, shortcut, side = 'top', disabled = false, class: className = 'inline-flex', children }: Props = $props();

  const SHOW_DELAY_MS = 350;
  /** Focus that lands right after the trigger appears is the app moving focus into a new dialog, not the user tabbing. */
  const SETTLE_MS = 300;

  const mountedAt = performance.now();

  const id = uid('tooltip');
  let anchor: HTMLElement | undefined = $state();
  let visible = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  /**
   * The label lives in `document.body` (see `portal`) so no `overflow` ancestor, such as a dialog, can
   * clip it; this places it next to the trigger and keeps it inside the window.
   */
  function place(node: HTMLElement): void {
    if (!anchor) {
      return;
    }

    const { left, top } = floatingPosition(
      anchor.getBoundingClientRect(),
      { width: node.offsetWidth, height: node.offsetHeight },
      side,
      { width: window.innerWidth, height: window.innerHeight },
    );

    node.style.left = `${left}px`;
    node.style.top = `${top}px`;
  }

  function showSoon(): void {
    clearTimeout(timer);
    if (disabled) {
      return;
    }

    timer = setTimeout(() => (visible = true), SHOW_DELAY_MS);
  }

  function showNow(): void {
    clearTimeout(timer);
    if (disabled) {
      return;
    }

    visible = true;
  }

  // Only keyboard focus reveals the tooltip; clicks and focus placed by a freshly opened dialog would just add noise.
  function onFocusIn(event: FocusEvent): void {
    if (performance.now() - mountedAt < SETTLE_MS) {
      return;
    }

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
  bind:this={anchor}
  class={cx('relative', className)}
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
      use:portal
      use:place
      transition:rise={{ y: 4, duration: 140 }}
      class={cx(
        'pointer-events-none fixed z-[85] rounded-sm border border-line-strong bg-surface-4 px-2 py-1 text-xs text-fg shadow-float',
        rich ? 'block w-max max-w-[340px] px-3.5 py-3 text-sm leading-snug' : detail ? 'flex max-w-[280px] flex-col gap-0.5 py-1.5' : 'inline-flex items-center gap-2 font-medium whitespace-nowrap',
      )}
    >
      {#if rich}
        {@render rich()}
      {:else if detail}
        <span class="flex items-center gap-2 font-medium">{text}{#if shortcut}<Kbd keys={shortcut} />{/if}</span>
        <span class="font-normal whitespace-pre-line text-fg-3">{detail}</span>
      {:else}
        {text}
        {#if shortcut}<Kbd keys={shortcut} />{/if}
      {/if}
    </span>
  {/if}
</span>
