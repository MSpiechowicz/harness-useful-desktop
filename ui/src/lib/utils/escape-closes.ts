/** Escape closes the panel that is open, once, and nothing else reacts to the key. */
export function escapeCloses(event: KeyboardEvent, open: boolean, close: () => void): void {
  if (event.key === 'Escape' && open && !event.defaultPrevented) {
    event.preventDefault();
    event.stopPropagation();
    close();
  }
}
