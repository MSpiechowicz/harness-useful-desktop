/** Whether the command palette is open; the search button in every view header and the shell's shortcuts share it. */
export class PaletteState {
  open = $state(false);

  toggle(): void {
    this.open = !this.open;
  }
}

export const palette = new PaletteState();
