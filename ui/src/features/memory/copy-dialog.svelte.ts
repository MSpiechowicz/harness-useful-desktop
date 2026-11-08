import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';

/**
 * What a view of notes or scripts remembers about its copy dialog: the banner's copy of what is stranded on this computer, or the Sync dialog. One
 * dialog serves both, so it is open when either was asked for.
 */
export class CopyDialogState {
  /** The banner asked to copy what is stranded on this computer to the remote database. */
  copying = $state(false);

  /**
   * The ways the Sync dialog open now copies, as they were when it was opened: Settings or an outage changing meanwhile does not turn it into another
   * copy. Null while it is closed.
   */
  syncing = $state.raw<readonly NotesCopyMode[] | null>(null);

  get open(): boolean {
    return this.copying || this.syncing !== null;
  }

  close(): void {
    this.copying = false;
    this.syncing = null;
  }
}
