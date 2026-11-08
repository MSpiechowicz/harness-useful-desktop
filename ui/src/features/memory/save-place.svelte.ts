import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';

/** What the Save to field of an editor depends on. */
export interface SavePlaceInput {
  /** The editor writes a new entry itself: not an edit, not a replace, and not the review card that hands the draft to its caller. */
  adding: boolean;
  /** The settings of the installed Harness Memory. Null until they are read. */
  remote: MemoryRemote | null;
  /** The installed Harness Memory takes a `place` for this kind of save (`savePlaceSupported` for an editor, `heldSavePlaceSupported` for a held save). */
  supported: boolean;
  /** Where the scope the entry goes to is kept. */
  routing: MemoryNotesRouting;
  /** The list on screen is the copy kept on this computer: the remote cannot be reached now. */
  offline: boolean;
}

/**
 * The Save to field of the editor of a new note or script, and of the approval card of a note or script an agent holds: whether it shows, and which side a save goes to. It shows only for a scope kept in
 * "Remote + this computer", where the installed Harness Memory takes a `place`. It starts on the Settings choice (kept in the Harness Memory config) each time the editor
 * opens and does not remember the last choice. While the remote cannot be reached the choice is this computer for that open, and the saved choice is left alone.
 */
export class SavePlace {
  /** What the user picked since the editor opened. Null until then, so the field follows the Settings choice even when the settings are read after the editor opened. */
  private chosen = $state<MemorySavePlace | null>(null);

  constructor(private readonly input: () => SavePlaceInput) {}

  /** Starts on the Settings choice again. */
  open(): void {
    this.chosen = null;
  }

  get offered(): boolean {
    const { adding, remote, supported, routing } = this.input();

    return adding && remote !== null && supported && routing === 'remoteLocal';
  }

  /** The remote cannot be reached, so only this computer can be chosen. */
  get offline(): boolean {
    return this.input().offline;
  }

  get value(): MemorySavePlace {
    if (this.offline) {
      return 'local';
    }

    return this.chosen ?? this.input().remote?.remoteLocalSaves ?? 'remote';
  }

  set value(next: MemorySavePlace) {
    this.chosen = next;
  }

  /**
   * What a save names: only a place the user picked while the field shows (the server refuses a place for an edit or a replace). Otherwise it names none, and the server follows
   * the Settings choice: a title kept on this computer is updated there, and while the remote cannot be reached the entry is saved here and copied to the remote once it is back.
   */
  get sent(): MemorySavePlace | undefined {
    if (!this.offered || this.offline) {
      return undefined;
    }

    return this.chosen ?? undefined;
  }
}
