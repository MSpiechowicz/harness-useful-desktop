import type { NoteContent } from '$lib/ipc/bindings/NoteContent';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';

/**
 * Whether an editor is open and what it edits. The header, the empty state, the panel and the command palette all open the same editor,
 * and it outlives none of them, so its state is kept here rather than in any one of them.
 */
class EditorSlot<T> {
  open = $state(false);
  /** What the editor was opened on; null for a new item. */
  target = $state.raw<T | null>(null);
  /** The command palette asked for a new item: the view opens the editor once it is up and can save. */
  wantsNew = $state(false);

  create(): void {
    this.target = null;
    this.open = true;
  }

  edit(item: T): void {
    this.target = item;
    this.open = true;
  }

  reset(): void {
    this.open = false;
    this.target = null;
    this.wantsNew = false;
  }
}

export const noteEditor = new EditorSlot<NoteContent>();
export const scriptEditor = new EditorSlot<ScriptDetail>();
