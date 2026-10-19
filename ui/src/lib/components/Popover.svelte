<script lang="ts">
  import type { Snippet } from 'svelte';

  import { pop } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';
  import { rovingIndex } from '$lib/utils/roving';

  interface TriggerProps {
    'aria-haspopup': 'menu' | 'dialog';
    'aria-expanded': boolean;
    'aria-controls': string;
    'data-popover-trigger': '';
    onclick: () => void;
  }

  interface Props {
    /** Accessible name of the popover panel. */
    label: string;
    /** `menu` for a list of `[data-menu-item]` actions; `dialog` for free-form content. */
    kind?: 'menu' | 'dialog';
    align?: 'start' | 'end';
    /** Which side of the trigger the panel opens on; `top` for triggers near the bottom of the window, `auto` for the side with more room inside the scrolling area around the trigger (a form in a dialog). */
    side?: 'top' | 'bottom' | 'auto';
    open?: boolean;
    /** Classes of the panel. */
    class?: string;
    /** Classes of the wrapper around trigger and panel, e.g. `w-full` for a trigger that fills its row. */
    rootClass?: string;
    /** Receives the attributes to spread onto the trigger button. */
    trigger: Snippet<[{ props: TriggerProps; open: boolean }]>;
    children: Snippet<[{ close: () => void }]>;
  }

  let { label, kind = 'menu', align = 'start', side = 'bottom', open = $bindable(false), class: className, rootClass, trigger, children }: Props = $props();

  const panelId = uid('popover');

  let root: HTMLElement | undefined = $state();
  let panel: HTMLElement | undefined = $state();
  /** Horizontal nudge and height cap that keep the open panel inside the window. */
  let shift = $state(0);
  let maxHeight = $state<number | null>(null);
  /** The side the panel opened on, `side` resolved when it is `auto`. */
  let placed = $state<'top' | 'bottom'>('bottom');
  /** Where an `auto` panel sits: fixed to the window, so a scrolling form around the trigger never cuts it off. */
  let fixed = $state<{ top: number; left: number } | null>(null);
  /** The trigger's width an `auto` panel takes from its first render, so it is measured at the width it shows at. */
  let anchorWidth = $state<number | null>(null);

  const EDGE = 8;

  const triggerProps = $derived<TriggerProps>({
    'aria-haspopup': kind,
    'aria-expanded': open,
    'aria-controls': panelId,
    'data-popover-trigger': '',
    onclick: () => (open = !open),
  });

  /** Where `position: fixed` measures from: the window, or the nearest ancestor a transform or filter (a glass dialog) turns into the frame. */
  function fixedOrigin(element: HTMLElement): { top: number; left: number } {
    for (let node = element.parentElement; node; node = node.parentElement) {
      const style = getComputedStyle(node);
      if (style.transform !== 'none' || style.filter !== 'none' || style.backdropFilter !== 'none' || style.getPropertyValue('-webkit-backdrop-filter') !== 'none' && style.getPropertyValue('-webkit-backdrop-filter') !== '') {
        const rect = node.getBoundingClientRect();

        return { top: rect.top, left: rect.left };
      }
    }

    return { top: 0, left: 0 };
  }

  function items(): HTMLElement[] {
    return Array.from(panel?.querySelectorAll<HTMLElement>('[data-menu-item]') ?? []);
  }

  function close(returnFocus: boolean): void {
    open = false;

    if (returnFocus) {
      root?.querySelector<HTMLElement>('[data-popover-trigger]')?.focus();
    }
  }

  // On open, move focus to a `[data-autofocus]` element (e.g. a filter field), else the first action, else the panel.
  $effect(() => {
    if (open) {
      queueMicrotask(() => (panel?.querySelector<HTMLElement>('[data-autofocus]') ?? items()[0] ?? panel)?.focus({ preventScroll: true }));
    }
  });

  // Runs before the panel is inserted, so an `auto` panel has the trigger's width before it is measured.
  $effect.pre(() => {
    anchorWidth = open && side === 'auto' && root ? root.getBoundingClientRect().width : null;
  });

  /** The panel's height with nothing capping it: its content and padding plus its border, which `max-height` also counts. */
  function naturalHeight(element: HTMLElement): number {
    const style = getComputedStyle(element);
    const border = (parseFloat(style.borderTopWidth) || 0) + (parseFloat(style.borderBottomWidth) || 0);

    return element.scrollHeight + border;
  }

  // Measured from layout sizes, not the panel's rect, since the open transition scales it.
  $effect(() => {
    if (!open || !panel || !root) {
      shift = 0;
      maxHeight = null;
      fixed = null;

      return;
    }

    const anchor = root.getBoundingClientRect();
    const width = Math.min(panel.offsetWidth, window.innerWidth - EDGE * 2);
    const left = align === 'end' ? anchor.right - width : anchor.left;
    const clamped = Math.min(Math.max(left, EDGE), window.innerWidth - EDGE - width);
    shift = clamped - left;

    const below = window.innerHeight - anchor.bottom;
    const above = anchor.top;
    const natural = naturalHeight(panel);
    placed = side !== 'auto' ? side : below >= natural + EDGE * 2 || below >= above ? 'bottom' : 'top';
    const room = placed === 'top' ? above : below;
    maxHeight = Math.max(room - EDGE * 2, 120);

    // A `top` panel ends a gap above the trigger: its height is the smaller of its content and the cap, never more.
    if (side === 'auto') {
      const origin = fixedOrigin(root);
      const height = Math.min(natural, maxHeight);
      const top = placed === 'top' ? anchor.top - EDGE - height : anchor.bottom + EDGE;
      fixed = { top: top - origin.top, left: clamped - origin.left };
      shift = 0;
    }
  });

  // Pointer interaction outside the popover dismisses it.
  $effect(() => {
    if (!open) {
      return;
    }

    const onPointerDown = (event: PointerEvent): void => {
      if (root && event.target instanceof Node && !root.contains(event.target)) {
        close(false);
      }
    };

    // A panel fixed to the window would drift away from its trigger, so scrolling the page under it or resizing closes it, as a native select does.
    const onMove = (event: Event): void => {
      if (!(event.target instanceof Node && panel?.contains(event.target))) {
        close(false);
      }
    };

    window.addEventListener('pointerdown', onPointerDown, true);
    if (side === 'auto') {
      window.addEventListener('scroll', onMove, true);
      window.addEventListener('resize', onMove);
    }

    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true);
      window.removeEventListener('scroll', onMove, true);
      window.removeEventListener('resize', onMove);
    };
  });

  function onKeydown(event: KeyboardEvent): void {
    if (!open) {
      return;
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close(true);

      return;
    }

    const list = items();
    const index = list.indexOf(document.activeElement as HTMLElement);
    const next = list.length === 0 ? null : rovingIndex(event.key, index, list.length, 'vertical');
    if (next !== null) {
      event.preventDefault();
      list[next].focus();
    }
  }

  // Tabbing out of the panel closes it.
  function onFocusOut(event: FocusEvent): void {
    if (open && root && event.relatedTarget instanceof Node && !root.contains(event.relatedTarget)) {
      close(false);
    }
  }
</script>

<div bind:this={root} class={cx('relative inline-flex min-w-0', rootClass)} onkeydown={onKeydown} onfocusout={onFocusOut} role="presentation">
  {@render trigger({ props: triggerProps, open })}

  {#if open}
    <div
      bind:this={panel}
      id={panelId}
      role={kind}
      aria-label={label}
      tabindex="-1"
      transition:pop={{ duration: 180, from: 0.97 }}
      style:translate={shift ? `${shift}px 0` : null}
      style:max-height={maxHeight === null ? null : `${maxHeight}px`}
      style:position={side === 'auto' ? 'fixed' : null}
      style:top={fixed ? `${fixed.top}px` : null}
      style:left={fixed ? `${fixed.left}px` : null}
      style:width={anchorWidth === null ? null : `${anchorWidth}px`}
      style:visibility={side === 'auto' && !fixed ? 'hidden' : null}
      class={cx(
        'absolute z-50 max-w-[calc(100vw-1rem)] min-w-64 overflow-y-auto rounded-lg border border-line-strong bg-glass p-1.5 shadow-pop backdrop-blur-xl outline-none',
        side === 'auto' ? (placed === 'top' ? 'origin-bottom' : 'origin-top') : side === 'top' ? 'bottom-full mb-2 origin-bottom' : 'top-full mt-2 origin-top',
        side !== 'auto' && (align === 'end' ? 'right-0' : 'left-0'),
        className,
      )}
    >
      {@render children({ close: () => close(true) })}
    </div>
  {/if}
</div>
