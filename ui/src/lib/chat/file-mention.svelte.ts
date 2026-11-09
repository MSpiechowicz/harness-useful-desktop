import { ipc } from '$lib/ipc/client';
import type { ProjectFile } from '$lib/ipc/bindings/ProjectFile';

import { type FileMention, fileMentionQuery } from './file-mention';

/** How long typing pauses before the project is asked, so a fast typist sends one request, not one per key. */
const DEBOUNCE_MS = 80;
/** Rows the menu shows. The backend clamps this to 1..=50. */
const LIMIT = 20;

/** The project and the text after the `@` that a set of rows answers. */
interface Ask {
  project: string;
  query: string;
}

function sameAsk(a: Ask | null, project: string, query: string): boolean {
  return a !== null && a.project === project && a.query === query;
}

/**
 * The rows of the `@` menu. A request waits for a short pause in typing, and a reply that arrives after a newer request was made is
 * dropped. The rows stay until the next reply replaces them, so the menu does not flicker while typing, which also means they can be
 * the answer to an older query: `isFresh` tells. A failure or no match leaves no rows, which closes the menu.
 */
export class FileSuggestions {
  rows = $state.raw<ProjectFile[]>([]);

  private timer: ReturnType<typeof setTimeout> | undefined;
  private latest = 0;
  /** What the rows answer. */
  private answered: Ask | null = null;
  /** The ask whose first row the user already chose with Enter or Tab, taken as soon as its reply has one. */
  private waiting: Ask | null = null;

  /** `onfirst` takes the first row of the reply a pending accept waited for. */
  constructor(private onfirst: (file: ProjectFile, query: string) => void) {}

  /** Asks `project` for the files matching `query` once typing pauses. */
  request(project: string, query: string): void {
    clearTimeout(this.timer);

    if (!sameAsk(this.waiting, project, query)) {
      this.waiting = null;
    }

    const id = ++this.latest;
    this.timer = setTimeout(() => void this.load(project, query, id), DEBOUNCE_MS);
  }

  /**
   * The row Enter or Tab takes. While the rows still answer an older query that is none: the first row of the reply to this query is
   * taken when it arrives (a reply with no rows takes nothing), unless the user moves on first.
   */
  choose(project: string, query: string, index: number): ProjectFile | null {
    if (sameAsk(this.answered, project, query)) {
      return this.rows[index] ?? null;
    }

    this.waiting = { project, query };

    return null;
  }

  /** Drops a pending accept, for when the user has moved on. */
  cancelAccept(): void {
    this.waiting = null;
  }

  /** Forgets the rows, any request still on its way, and any pending accept. */
  clear(): void {
    clearTimeout(this.timer);
    this.latest += 1;
    this.answered = null;
    this.waiting = null;

    if (this.rows.length > 0) {
      this.rows = [];
    }
  }

  private async load(project: string, query: string, id: number): Promise<void> {
    let files: ProjectFile[];

    try {
      files = await ipc.invoke('project_files', { project, query, limit: LIMIT });
    } catch {
      files = [];
    }

    if (id !== this.latest) {
      return;
    }

    this.rows = files;
    this.answered = { project, query };

    const accepting = sameAsk(this.waiting, project, query);
    this.waiting = null;

    if (accepting && files.length > 0) {
      this.onfirst(files[0], query);
    }
  }
}

/**
 * Where the user's caret was at their last touch of the textarea, and the text it was in. A draft that code put there does not match,
 * so it never opens the file menu, whatever its text: code clears the caret when it writes (`forget`), and a draft changed by anyone
 * else drops it at the next `keepFor`.
 */
export class DraftCaret {
  private at = $state.raw<{ value: string; start: number; end: number } | null>(null);

  /** Notes the caret of `box`, but only for the text the draft holds: a selection event that arrives before the draft has taken the new text is not a caret to keep. */
  track(box: HTMLTextAreaElement, draft: string): void {
    const { value, selectionStart: start, selectionEnd: end } = box;

    if (value !== draft || (this.at?.value === value && this.at.start === start && this.at.end === end)) {
      return;
    }

    this.at = { value, start, end };
  }

  /** The file mention the caret is in for `draft`, or null for a draft the user did not type. */
  mentionIn(draft: string): FileMention | null {
    const at = this.at;

    return at && at.value === draft ? fileMentionQuery(draft, at.start, at.end) : null;
  }

  /** Forgets the caret, for text that code puts in the draft. */
  forget(): void {
    this.at = null;
  }

  /** Forgets the caret when the draft is no longer the text it was in. */
  keepFor(draft: string): void {
    if (this.at && this.at.value !== draft) {
      this.at = null;
    }
  }
}
