/** What an editor of a note or a script keeps of the item it opened on, to save an edit against that version and to say when it is not the latest. */
export class OpenedVersion<T extends { id: number }> {
  /** The version the form was filled from: the one opened, or the one the user reloaded. */
  opened = $state.raw<T | null>(null);

  /** The last save found the item changed (`changed`) or deleted (`gone`) since `opened` was read. */
  stale = $state<'changed' | 'gone' | null>(null);

  reloading = $state(false);

  /** Raised by `open`, so a read that was started on an earlier item is dropped when it answers. */
  private opening = 0;

  /** Starts on `item` (null for an add), with nothing found stale. A read still running for the item before is dropped. */
  open(item: T | null): void {
    this.opening += 1;
    this.opened = item;
    this.stale = null;
    this.reloading = false;
  }

  /** Reads the item again with `read`: `fill` gets the latest version to put in the form, which drops the edits made on the old one. `gone` is said when it no longer exists. */
  async reload(read: (id: number) => Promise<T | 'gone' | null>, fill: (latest: T) => void): Promise<void> {
    if (this.opened === null || this.reloading) {
      return;
    }

    const opening = this.opening;
    this.reloading = true;

    try {
      const latest = await read(this.opened.id);

      if (opening !== this.opening) {
        return;
      }

      if (latest === 'gone') {
        this.stale = 'gone';
      } else if (latest !== null) {
        this.opened = latest;
        fill(latest);
        this.stale = null;
      }
    } finally {
      if (opening === this.opening) {
        this.reloading = false;
      }
    }
  }
}
