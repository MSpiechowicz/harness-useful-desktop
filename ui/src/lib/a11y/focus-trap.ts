import type { Action } from 'svelte/action';

export interface FocusTrapOptions {
  /** Called when Escape is pressed inside the trap. */
  onEscape?: () => void;
  /** Selector for the element to focus first; defaults to the first tabbable. */
  initialFocus?: string;
}

const TABBABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function tabbables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(TABBABLE)).filter((element) => !element.closest('[hidden], [inert]'));
}

/** Preferred first focus: the requested element, else the first control that is not a dismiss (close) button. */
function focusInitial(root: HTMLElement, selector: string | undefined): void {
  const items = tabbables(root);
  const preferred = selector ? root.querySelector<HTMLElement>(selector) : null;
  const firstNonDismiss = items.find((element) => !element.hasAttribute('data-dismiss'));
  const target = preferred ?? firstNonDismiss ?? items[0] ?? root;

  target.focus({ preventScroll: true });
}

function wrapTab(event: KeyboardEvent, root: HTMLElement): void {
  const items = tabbables(root);
  if (items.length === 0) {
    event.preventDefault();
    root.focus();

    return;
  }

  const first = items[0];
  const last = items[items.length - 1];
  const active = document.activeElement;

  if (event.shiftKey && (active === first || active === root)) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
  }
}

/**
 * Traps Tab inside the node, closes on Escape, and returns focus to whatever
 * was focused before the node mounted when it is destroyed.
 */
export const focusTrap: Action<HTMLElement, FocusTrapOptions | undefined> = (node, options) => {
  let current = options;
  const previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  node.tabIndex = -1;

  // Deferred so ancestors' actions (e.g. `portal`, which moves the node and would drop focus) have run.
  queueMicrotask(() => {
    if (node.isConnected) {
      focusInitial(node, current?.initialFocus);
    }
  });

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && current?.onEscape) {
      event.preventDefault();
      event.stopPropagation();
      current.onEscape();

      return;
    }
    if (event.key === 'Tab') {
      wrapTab(event, node);
    }
  }

  node.addEventListener('keydown', onKeydown);

  return {
    update(next) {
      current = next;
    },
    destroy() {
      node.removeEventListener('keydown', onKeydown);

      if (previouslyFocused?.isConnected) {
        previouslyFocused.focus({ preventScroll: true });
      }
    },
  };
};
