import { t, tn } from '$lib/i18n/i18n.svelte';
import type { LocalNoteIds } from '$lib/ipc/bindings/LocalNoteIds';
import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import type { NotesCopyFilter } from '$lib/ipc/bindings/NotesCopyFilter';
import type { NotesCopyPreview } from '$lib/ipc/bindings/NotesCopyPreview';
import type { NotesCopyResult } from '$lib/ipc/bindings/NotesCopyResult';
import type { NotesCopyRow } from '$lib/ipc/bindings/NotesCopyRow';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { toasts } from '$lib/stores/toasts.svelte';

import { COPY_TEXT } from './notes-copy-text';
import { listOf } from './notes-discard';

/** What the dialog shows: the comparison running, its review, the copy running, what it did, or why the comparison failed. */
export type CopyStage = 'loading' | 'review' | 'applying' | 'result' | 'failed';

/** How many rows one page of the dialog's tables holds. */
export const COPY_PAGE_SIZE = 50;

/** How many conflicts one read fetches while the ids of all of them are collected. */
const COLLECT_LIMIT = 200;

/**
 * Copying the notes (or the scripts, per `kind`) saved on this computer to the remote database. `begin` asks Rust for a dry run, which it keeps
 * under `previewId`, and the dialog shows its counts and the conflicts (entries the remote also has, differing from this computer's newer one) a
 * page at a time. Conflicts start unticked: only a ticked one overwrites the teammates' version. Blocked entries (the copy would fail, say
 * because the remote has one of the same name hidden on this computer) are listed apart with their reason: they are never ticked, never counted
 * as copied, and never sent. Copying sends the plan's id and the ticks, never the entries.
 */
export class NotesCopyFlow {
  constructor(readonly kind: MemoryCopyKind) {}

  stage = $state<CopyStage>('loading');
  preview = $state.raw<NotesCopyPreview | null>(null);
  result = $state.raw<NotesCopyResult | null>(null);
  failure = $state<string | null>(null);

  /** The conflicts of the page on screen, grouped by repository by the server. */
  rows = $state.raw<NotesCopyRow[]>([]);
  page = $state(0);
  /** Whether the page is being read. */
  paging = $state(false);

  /** The blocked entries of the page on screen, with the reason each one cannot be copied. */
  blockedRows = $state.raw<NotesCopyRow[]>([]);
  blockedPage = $state(0);
  blockedPaging = $state(false);

  /** The ticked conflicts, by local id. With `every` set, all conflicts are ticked except the ones listed in `unticked`. */
  private ticked = $state.raw<Set<number>>(new Set());
  private every = $state(false);
  private unticked = $state.raw<Set<number>>(new Set());

  /** Raised by `reset`, so an answer that arrives after the dialog was closed is dropped. */
  private generation = 0;

  get conflicts(): number {
    return this.preview?.counts.conflicts ?? 0;
  }

  /** How many entries cannot be copied. */
  get blocked(): number {
    return this.preview?.counts.failed ?? 0;
  }

  /** How many entries the copy takes: all but the blocked ones. */
  get copyable(): number {
    return (this.preview?.total ?? 0) - this.blocked;
  }

  /** How many conflicts will overwrite the team's version. */
  get pickedCount(): number {
    return this.every ? this.conflicts - this.unticked.size : this.ticked.size;
  }

  isPicked(localId: number): boolean {
    return this.every ? !this.unticked.has(localId) : this.ticked.has(localId);
  }

  /** Compares this computer with the team database (it can take long with many notes) and reads the first page of conflicts. */
  async begin(): Promise<void> {
    const generation = this.generation;
    this.stage = 'loading';
    this.failure = null;
    this.preview = null;

    try {
      const preview = await ipc.invoke('memory_notes_copy_preview', { kind: this.kind });

      if (generation !== this.generation) {
        return;
      }

      this.preview = preview;
      this.rows = [];
      this.page = 0;
      this.blockedRows = [];
      this.blockedPage = 0;
      this.every = false;
      this.ticked = new Set();
      this.unticked = new Set();

      await Promise.all([preview.counts.conflicts > 0 ? this.go(0) : undefined, preview.counts.failed > 0 ? this.goBlocked(0) : undefined]);

      if (generation === this.generation) {
        this.stage = 'review';
      }
    } catch (error) {
      if (generation === this.generation) {
        this.failure = IpcFailure.from(error).message;
        this.stage = 'failed';
      }
    }
  }

  /** Reads one page of the conflicts. */
  async go(page: number): Promise<void> {
    this.paging = true;

    try {
      const rows = await this.read('conflicts', page);

      if (rows) {
        this.rows = rows;
        this.page = page;
      }
    } finally {
      this.paging = false;
    }
  }

  /** Reads one page of the blocked entries. */
  async goBlocked(page: number): Promise<void> {
    this.blockedPaging = true;

    try {
      const rows = await this.read('failed', page);

      if (rows) {
        this.blockedRows = rows;
        this.blockedPage = page;
      }
    } finally {
      this.blockedPaging = false;
    }
  }

  /** One page of the rows under a filter, or nothing when the dialog was reset meanwhile. */
  private async read(filter: NotesCopyFilter, page: number): Promise<NotesCopyRow[] | undefined> {
    const previewId = this.preview?.previewId;

    if (!previewId) {
      return undefined;
    }

    const generation = this.generation;
    const { rows } = await ipc.invoke('memory_notes_copy_rows', { kind: this.kind, previewId, filter, offset: page * COPY_PAGE_SIZE, limit: COPY_PAGE_SIZE });

    return generation === this.generation ? rows : undefined;
  }

  toggle(localId: number, on: boolean): void {
    const next = new Set(this.every ? this.unticked : this.ticked);

    if (on === this.every) {
      next.delete(localId);
    } else {
      next.add(localId);
    }

    if (this.every) {
      this.unticked = next;
    } else {
      this.ticked = next;
    }
  }

  /** Ticks every conflict on every page, or none. */
  toggleAll(on: boolean): void {
    this.every = on;
    this.ticked = new Set();
    this.unticked = new Set();
  }

  /** What `memory_notes_copy_apply` takes: none, all, or the local ids ticked (collected from every page when some of all were unticked). */
  private async overwrite(previewId: string): Promise<LocalNoteIds> {
    if (!this.every) {
      return this.ticked.size === 0 ? 'none' : [...this.ticked];
    }

    if (this.unticked.size === 0) {
      return 'all';
    }

    const ids: number[] = [];

    for (let offset = 0; ids.length < this.conflicts + this.unticked.size; offset += COLLECT_LIMIT) {
      const { rows } = await ipc.invoke('memory_notes_copy_rows', { kind: this.kind, previewId, filter: 'conflicts', offset, limit: COLLECT_LIMIT });

      ids.push(...rows.filter((row) => !row.failed).map((row) => row.localId));

      if (rows.length < COLLECT_LIMIT) {
        break;
      }
    }

    return ids.filter((id) => !this.unticked.has(id));
  }

  /** Copies, then says what happened in a toast and in the dialog. */
  async apply(): Promise<void> {
    const previewId = this.preview?.previewId;

    if (!previewId || this.stage !== 'review') {
      return;
    }

    const generation = this.generation;
    this.stage = 'applying';
    this.failure = null;

    try {
      const result = await ipc.invoke('memory_notes_copy_apply', { kind: this.kind, previewId, overwrite: await this.overwrite(previewId) });

      if (generation === this.generation) {
        this.result = result;
        this.stage = 'result';
        this.say(result);
      }
    } catch (error) {
      if (generation === this.generation) {
        this.failure = IpcFailure.from(error).message;
        this.stage = 'failed';
      }
    }

    // Entries copied before a failure are copied, so the list is read again either way.
    await listOf(this.kind).refresh({ quiet: true });
  }

  /** What the copy left on purpose is deleted from this computer: it stays no more. */
  discarded(): void {
    if (this.result) {
      this.result = { ...this.result, kept: Math.max(0, this.result.kept - this.result.leftover.length), leftover: [] };
    }
  }

  private say(result: NotesCopyResult): void {
    const stays = result.kept;
    const text = COPY_TEXT[this.kind];

    toasts.push({
      title: tn(text.toastDone, copiedCount(result)),
      description: stays > 0 ? tn(text.toastStays, stays) : undefined,
      tone: result.failed.length > 0 ? 'warning' : 'success',
    });
  }

  reset(): void {
    this.generation += 1;
    this.stage = 'loading';
    this.preview = null;
    this.result = null;
    this.failure = null;
    this.rows = [];
    this.page = 0;
    this.paging = false;
    this.blockedRows = [];
    this.blockedPage = 0;
    this.blockedPaging = false;
    this.toggleAll(false);
  }
}

/** The notes now in the team database: copied, overwritten or already there. Not the ones deleted here, a failed relation deletes nothing. */
export function copiedCount({ counts }: NotesCopyResult): number {
  return counts.added + counts.updated + counts.overwritten + counts.identical;
}

/** The notes that failed, each counted once. A failed note comes with one failure per relation, all with its local id, and a failure without a local id is no note. */
export function failedNotes({ failed }: NotesCopyResult): number {
  return new Set(failed.map((failure) => failure.localId).filter((id) => id !== null)).size;
}

/** The title of the dialog for what it is showing. */
export function copyTitle(flow: NotesCopyFlow): string {
  const text = COPY_TEXT[flow.kind];

  switch (flow.stage) {
    case 'applying':
      return t(text.titleApplying);
    case 'result':
      return t('memory.copy.title.done');
    case 'failed':
      return t('memory.copy.title.failed');
    case 'loading':
      return t(text.titleLoading);
    case 'review':
      if (flow.preview && flow.preview.total > 0) {
        return flow.copyable > 0 ? tn(text.title, flow.copyable) : tn(text.blockedTitle, flow.blocked);
      }

      return t('memory.copy.title.none');
  }
}

/** The line under the title. */
export function copyDescription(flow: NotesCopyFlow): string {
  const text = COPY_TEXT[flow.kind];

  switch (flow.stage) {
    case 'applying':
      return t('memory.copy.description.applying');
    case 'result':
      return t(text.descriptionDone);
    case 'failed':
      return flow.failure ?? '';
    case 'loading':
      return t(text.loadingHint);
    case 'review':
      if (flow.preview && flow.preview.total > 0) {
        return t(flow.copyable > 0 ? text.description : text.blockedHelp);
      }

      return t(text.descriptionNone);
  }
}
