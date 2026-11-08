import { t, tn } from '$lib/i18n/i18n.svelte';
import type { LocalNoteIds } from '$lib/ipc/bindings/LocalNoteIds';
import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import type { NotesCopyFilter } from '$lib/ipc/bindings/NotesCopyFilter';
import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';
import type { NotesCopyPreview } from '$lib/ipc/bindings/NotesCopyPreview';
import type { NotesCopyResult } from '$lib/ipc/bindings/NotesCopyResult';
import type { NotesCopyRow } from '$lib/ipc/bindings/NotesCopyRow';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { toasts } from '$lib/stores/toasts.svelte';

import { copiedCount } from './notes-copy-headings';
import { COPY_TEXT } from './notes-copy-text';
import { listOf } from './notes-discard';

/** What the dialog shows: the comparison running, its review, the copy running, what it did, or why the comparison failed. */
export type CopyStage = 'loading' | 'review' | 'applying' | 'result' | 'failed';

/** How many rows one page of the dialog's tables holds. */
export const COPY_PAGE_SIZE = 50;

/** How many conflicts one read fetches while the ids of all of them are collected. */
const COLLECT_LIMIT = 200;

/**
 * The id a tick on `row` is made of: the local id of an entry copied to the remote (or of a conflict that overwrites the remote copy), the remote id
 * of an entry copied to this computer (it has no local id yet). Null for a row that has none, which cannot be ticked.
 */
export function tickId(row: NotesCopyRow, mode: NotesCopyMode): number | null {
  return mode === 'toLocal' ? row.teamId : row.localId;
}

/** A failure of the copy in words the user can act on: a copy that is turned off or that the installed server lacks says so, the rest is the server's own message. */
function failureText(error: unknown, mode: NotesCopyMode): string {
  const failure = IpcFailure.from(error);

  if (mode === 'stranded') {
    return failure.message;
  }

  if (failure.code === 'forbidden') {
    return t(mode === 'toLocal' ? 'memory.sync.forbidden.toLocal' : 'memory.sync.forbidden.toRemote');
  }

  return failure.code === 'unsupported' ? t('memory.sync.unsupported') : failure.message;
}

/**
 * Copying the notes (or the scripts, per `kind`) saved on this computer to the remote database. `begin` asks Rust for a dry run, which it keeps
 * under `previewId`, and the dialog shows its counts and the conflicts (entries the remote also has, differing from this computer's newer one) a
 * page at a time. Conflicts start unticked: only a ticked one overwrites the teammates' version. Blocked entries (the copy would fail, say
 * because the remote has one of the same name hidden on this computer) are listed apart with their reason: they are never ticked, never counted
 * as copied, and never sent. Copying sends the plan's id and the ticks, never the entries.
 *
 * `mode` is which way it goes. `stranded` copies everything this computer has stranded to the remote database, and what arrived is removed here.
 * `toRemote` and `toLocal` pick: nothing is ticked at first, only the entries ticked are copied, and Copy waits for a tick. `toRemote` copies
 * this computer's entries to the remote database and removes here what arrived, and a ticked conflict replaces the remote version. `toLocal`
 * copies the remote's entries missing here to this computer and removes nothing anywhere. A picking table lists the entries that can be ticked, or
 * every entry with the reason each cannot be (it is the same on both sides, the remote one is newer, a different version is here and is never
 * replaced, or it is blocked).
 */
export class NotesCopyFlow {
  constructor(
    readonly kind: MemoryCopyKind,
    readonly mode: NotesCopyMode = 'stranded',
  ) {}

  stage = $state<CopyStage>('loading');
  preview = $state.raw<NotesCopyPreview | null>(null);
  result = $state.raw<NotesCopyResult | null>(null);
  failure = $state<string | null>(null);

  /** Which rows the table lists: the conflicts, or when picking the entries that can be picked or all of them. */
  view = $state<NotesCopyFilter>('conflicts');

  /** The rows of the page on screen, grouped by repository by the server. */
  rows = $state.raw<NotesCopyRow[]>([]);
  page = $state(0);
  /** Whether the page is being read. */
  paging = $state(false);

  /** The blocked entries of the page on screen, with the reason each one cannot be copied. */
  blockedRows = $state.raw<NotesCopyRow[]>([]);
  blockedPage = $state(0);
  blockedPaging = $state(false);

  /** The ticked rows, by `tickId`. With `every` set, all that can be ticked are ticked except the ones listed in `unticked`. */
  private ticked = $state.raw<Set<number>>(new Set());
  private every = $state(false);
  private unticked = $state.raw<Set<number>>(new Set());

  /** Raised by `reset`, so an answer that arrives after the dialog was closed is dropped. */
  private generation = 0;

  /** Raised by every read of the table's page and of the blocked entries' page, so only the newest one may fill its rows. */
  private pageRequest = 0;
  private blockedRequest = 0;

  /** Whether the copy goes to this computer. */
  get pulls(): boolean {
    return this.mode === 'toLocal';
  }

  /** Whether the user picks the entries to copy (nothing is ticked at first), instead of the copy taking all and ticking only the conflicts. */
  get picking(): boolean {
    return this.mode !== 'stranded';
  }

  get conflicts(): number {
    return this.preview?.counts.conflicts ?? 0;
  }

  /** How many rows can be ticked: the conflicts, or when picking the entries the copy would add, update or replace. */
  get selectable(): number {
    const counts = this.preview?.counts;

    if (!counts) {
      return 0;
    }

    if (!this.picking) {
      return counts.conflicts;
    }

    return this.pulls ? counts.added : counts.added + counts.updated + counts.conflicts;
  }

  /** How many entries cannot be copied. */
  get blocked(): number {
    return this.preview?.counts.failed ?? 0;
  }

  /** How many entries the copy takes: all but the blocked ones, or when picking the ones that can be picked. */
  get copyable(): number {
    return this.picking ? this.selectable : (this.preview?.total ?? 0) - this.blocked;
  }

  /** How many rows are ticked: conflicts that will overwrite the remote version, or when picking the entries that will be copied. */
  get pickedCount(): number {
    return this.every ? this.selectable - this.unticked.size : this.ticked.size;
  }

  isPicked(id: number): boolean {
    return this.every ? !this.unticked.has(id) : this.ticked.has(id);
  }

  /** The mode the commands take: the default one is sent as none, as it always was. */
  private get modeArg(): { mode?: NotesCopyMode } {
    return this.mode === 'stranded' ? {} : { mode: this.mode };
  }

  /** The rows that carry the ticks. */
  private get tickFilter(): NotesCopyFilter {
    return this.picking ? 'pickable' : 'conflicts';
  }

  /** Compares this computer with the team database (it can take long with many notes) and reads the first page of rows. */
  async begin(): Promise<void> {
    const generation = this.generation;
    this.stage = 'loading';
    this.failure = null;
    this.preview = null;

    try {
      const preview = await ipc.invoke('memory_notes_copy_preview', { kind: this.kind, ...this.modeArg });

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

      // When picking the table starts with what can be picked, or with everything when nothing can.
      this.view = this.picking ? (this.selectable > 0 ? 'pickable' : 'all') : 'conflicts';

      const listed = this.picking ? preview.total > 0 : preview.counts.conflicts > 0;

      await Promise.all([listed ? this.go(0) : undefined, preview.counts.failed > 0 ? this.goBlocked(0) : undefined]);

      if (generation === this.generation) {
        this.stage = 'review';
      }
    } catch (error) {
      if (generation === this.generation) {
        this.failure = failureText(error, this.mode);
        this.stage = 'failed';
      }
    }
  }

  /** Reads one page of the table. An answer that arrives after a newer read of the table started (another page or filter) is dropped. */
  async go(page: number): Promise<void> {
    this.pageRequest += 1;
    const request = this.pageRequest;
    this.paging = true;

    try {
      const rows = await this.read(this.view, page);

      if (rows && request === this.pageRequest) {
        this.rows = rows;
        this.page = page;
      }
    } finally {
      if (request === this.pageRequest) {
        this.paging = false;
      }
    }
  }

  /** Switches a picking copy between the entries that can be picked and all of them, and reads the first page. Ticks stay. */
  async show(view: 'pickable' | 'all'): Promise<void> {
    this.view = view;
    this.page = 0;
    this.rows = [];

    await this.go(0);
  }

  /** Reads one page of the blocked entries, dropping an answer that a newer read of them has outdated. */
  async goBlocked(page: number): Promise<void> {
    this.blockedRequest += 1;
    const request = this.blockedRequest;
    this.blockedPaging = true;

    try {
      const rows = await this.read('failed', page);

      if (rows && request === this.blockedRequest) {
        this.blockedRows = rows;
        this.blockedPage = page;
      }
    } finally {
      if (request === this.blockedRequest) {
        this.blockedPaging = false;
      }
    }
  }

  /** One page of the rows under a filter, or nothing when the dialog was reset meanwhile. */
  private async read(filter: NotesCopyFilter, page: number): Promise<NotesCopyRow[] | undefined> {
    const previewId = this.preview?.previewId;

    if (!previewId) {
      return undefined;
    }

    const generation = this.generation;
    const { rows } = await ipc.invoke('memory_notes_copy_rows', { kind: this.kind, ...this.modeArg, previewId, filter, offset: page * COPY_PAGE_SIZE, limit: COPY_PAGE_SIZE });

    return generation === this.generation ? rows : undefined;
  }

  toggle(id: number, on: boolean): void {
    const next = new Set(this.every ? this.unticked : this.ticked);

    if (on === this.every) {
      next.delete(id);
    } else {
      next.add(id);
    }

    if (this.every) {
      this.unticked = next;
    } else {
      this.ticked = next;
    }
  }

  /** Ticks every row that can be ticked, on every page, or none. */
  toggleAll(on: boolean): void {
    this.every = on;
    this.ticked = new Set();
    this.unticked = new Set();
  }

  /** What `memory_notes_copy_apply` takes as its ticks: none, all, or the ids ticked (collected from every page when some of all were unticked). */
  private async picks(previewId: string): Promise<LocalNoteIds> {
    if (!this.every) {
      return this.ticked.size === 0 ? 'none' : [...this.ticked];
    }

    if (this.unticked.size === 0) {
      return 'all';
    }

    const ids: number[] = [];

    for (let offset = 0; ids.length < this.selectable + this.unticked.size; offset += COLLECT_LIMIT) {
      const { rows } = await ipc.invoke('memory_notes_copy_rows', { kind: this.kind, ...this.modeArg, previewId, filter: this.tickFilter, offset, limit: COLLECT_LIMIT });

      ids.push(...rows.filter((row) => !row.failed && row.pickable).flatMap((row) => tickId(row, this.mode) ?? []));

      if (rows.length < COLLECT_LIMIT) {
        break;
      }
    }

    return ids.filter((id) => !this.unticked.has(id));
  }

  /** Copies, then says what happened in a toast and in the dialog. */
  async apply(): Promise<void> {
    const previewId = this.preview?.previewId;

    // When picking only what is picked is copied: nothing picked is nothing to do.
    if (!previewId || this.stage !== 'review' || (this.picking && this.pickedCount === 0)) {
      return;
    }

    const generation = this.generation;
    this.stage = 'applying';
    this.failure = null;

    try {
      const picks = await this.picks(previewId);
      const base = { kind: this.kind, ...this.modeArg, previewId };
      const result = await ipc.invoke('memory_notes_copy_apply', this.picking ? { ...base, pick: picks } : { ...base, overwrite: picks });

      if (generation === this.generation) {
        this.result = result;
        this.stage = 'result';
        this.say(result);
      }
    } catch (error) {
      if (generation === this.generation) {
        this.failure = failureText(error, this.mode);
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
      title: t(text.toastDone),
      description: [tn(text.toastDoneBody, copiedCount(result, this.mode)), stays > 0 ? tn(text.toastStays, stays) : ''].filter(Boolean).join(' '),
      tone: result.failed.length > 0 ? 'warning' : 'success',
    });
  }

  reset(): void {
    this.generation += 1;
    this.stage = 'loading';
    this.preview = null;
    this.result = null;
    this.failure = null;
    this.view = 'conflicts';
    this.rows = [];
    this.page = 0;
    this.paging = false;
    this.blockedRows = [];
    this.blockedPage = 0;
    this.blockedPaging = false;
    this.toggleAll(false);
  }
}
