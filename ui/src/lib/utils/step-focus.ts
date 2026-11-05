import { tick } from 'svelte';

/** After a step that may have turned its own arrow off: moves the keyboard to the arrow still on, so it does not fall back to the page. */
export async function keepStepFocus(root: HTMLElement | undefined, arrows: string): Promise<void> {
  await tick();

  if (document.activeElement === document.body || document.activeElement?.matches(':disabled')) {
    root?.querySelector<HTMLElement>(`${arrows}:not(:disabled)`)?.focus();
  }
}
