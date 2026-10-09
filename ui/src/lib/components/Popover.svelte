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
    open?: boolean;
    /** Classes of the panel. */
    class?: string;
    /** Classes of the wrapper around trigger and panel, e.g. `w-full` for a trigger that fills its row. */
    rootClass?: string;
    /** Receives the attributes to spread onto the trigger button. */
    trigger: Snippet<[{ props: TriggerProps; open: boolean }]>;
    children: Snippet<[{ close: () => void }]>;
  }

  let { label, kind = 'menu', align = 'start', open = $bindable(false), class: className, rootClass, trigger, children }: Props = $props();

  const panelId = uid('popover');

  let root: HTMLElement | undefined = $state();
  let panel: HTMLElement | undefined = $state();

  const triggerProps = $derived<TriggerProps>({
    'aria-haspopup': kind,
    'aria-expanded': open,
    'aria-controls': panelId,
    'data-popover-trigger': '',
    onclick: () => (open = !open),
  });

  function items(): HTMLElement[] {
    return Array.from(panel?.querySelectorAll<HTMLElement>('[data-menu-item]') ?? []);
  }

  function close(returnFocus: boolean): void {
    open = false;

    if (returnFocus) {
      root?.querySelector<HTMLElement>('[data-popover-trigger]')?.focus();
    }
  }

  // On open, move focus to the first action (or the panel itself for free-form content).
  $effect(() => {
    if (open) {
      queueMicrotask(() => (items()[0] ?? panel)?.focus({ preventScroll: true }));
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

    window.addEventListener('pointerdown', onPointerDown, true);

    return () => window.removeEventListener('pointerdown', onPointerDown, true);
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
      class={cx(
        'absolute top-full z-50 mt-2 min-w-64 origin-top rounded-lg border border-line-strong bg-glass p-1.5 shadow-pop backdrop-blur-xl outline-none',
        align === 'end' ? 'right-0' : 'left-0',
        className,
      )}
    >
      {@render children({ close: () => close(true) })}
    </div>
  {/if}
</div>
