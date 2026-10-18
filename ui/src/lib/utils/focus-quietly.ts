/**
 * Moves focus without asking for the keyboard focus ring. The app moves focus by itself (back to a toggle when its
 * panel closes), and a ring there would look like something the user did with the keyboard. Where the browser does not
 * know the option, it focuses as usual, and the ring follows its own rule. Tab still shows the ring.
 */
export function focusQuietly(element: HTMLElement | null | undefined): void {
  // `focusVisible` is not in every DOM typing yet.
  element?.focus({ focusVisible: false } as FocusOptions);
}
