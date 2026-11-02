import { t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeEmbedding } from '$lib/ipc/bindings/KnowledgeEmbedding';
import type { KnowledgeFileStatus } from '$lib/ipc/bindings/KnowledgeFileStatus';
import type { KnowledgeImportCounts } from '$lib/ipc/bindings/KnowledgeImportCounts';
import type { KnowledgeImportEvent } from '$lib/ipc/bindings/KnowledgeImportEvent';
import type { KnowledgeIntake } from '$lib/ipc/bindings/KnowledgeIntake';
import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';
import type { KnowledgeSimilar } from '$lib/ipc/bindings/KnowledgeSimilar';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';
import { writeFailureText } from '$lib/utils/knowledge-failure';
import { choiceKey, type Landing, type MatchChoices, type MatchRow, matchRows, replacementsOf } from '$lib/utils/knowledge-match';

import { toasts } from './toasts.svelte';

/** One file of an import as the server reported it. */
export interface ImportRow {
  path: string;
  status: KnowledgeFileStatus;
  reason: string | null;
  chunks: number | null;
  embedding: KnowledgeEmbedding | null;
  warnings: string[];
}

/** What an import has done so far: built from its events by `reduceImport`. */
export interface ImportRun {
  /** The number of files, once the server planned them. */
  total: number | null;
  done: number;
  rows: ImportRow[];
  /** The closing counts, once the import ended. */
  counts: KnowledgeImportCounts | null;
  cancelled: boolean;
  embeddingPending: number;
  /** Why an import that could not run at all stopped. */
  failure: string | null;
  /** When the first file was waited for, in ms: the ETA counts from here. */
  startedAt: number;
}

export function emptyRun(startedAt: number): ImportRun {
  return { total: null, done: 0, rows: [], counts: null, cancelled: false, embeddingPending: 0, failure: null, startedAt };
}

/** The run after one more event of the import, in the order the server prints them. */
export function reduceImport(run: ImportRun, event: KnowledgeImportEvent): ImportRun {
  switch (event.type) {
    case 'plan':
      return { ...run, total: event.total };
    case 'file':
      return { ...run, rows: [...run.rows, { path: event.path, status: event.status, reason: event.reason, chunks: event.chunks, embedding: event.embedding, warnings: event.warnings }] };
    case 'progress':
      return { ...run, done: event.done, total: event.total };
    case 'done':
      return { ...run, counts: event.counts, cancelled: event.cancelled, embeddingPending: event.embeddingPending };
    case 'failed':
      return { ...run, failure: event.message };
  }
}

/** Seconds left at the pace so far, or null before a file is done (or once all are). */
export function etaSeconds(run: ImportRun, now: number): number | null {
  if (run.total === null || run.done === 0 || run.done >= run.total) {
    return null;
  }

  const perFile = (now - run.startedAt) / run.done;

  return Math.max(1, Math.round((perFile * (run.total - run.done)) / 1000));
}

/** How far the import is, 0 to 1; null while the number of files is not known. */
export function fractionOf(run: ImportRun): number | null {
  return run.total === null || run.total === 0 ? null : Math.min(1, run.done / run.total);
}

export type ImportPhase = 'idle' | 'staged' | 'running' | 'done';

/**
 * The import of files and folders into Harness Knowledge: what was picked or dropped (the intake), where it is linked to, and the run once it started.
 * It lives outside the dialog, so closing the window keeps the import running and the header can show its progress.
 */
export class KnowledgeImportStore {
  /** The dialog is open. */
  open = $state(false);
  phase = $state<ImportPhase>('idle');
  intake = $state.raw<KnowledgeIntake | null>(null);
  /** Where every imported document is linked to; none means global. */
  links = $state.raw<KnowledgeLink[]>([]);
  /** Where every imported document is kept when the user chose (both places in use), none for the routing's own choice. */
  place = $state<KnowledgePlace | null>(null);
  run = $state.raw<ImportRun>(emptyRun(0));
  /** A native dialog is open. */
  picking = $state(false);
  /** The user asked to stop and the server has not yet. */
  cancelling = $state(false);
  /** Chunks the last import left without a vector, which the notice offers to embed. */
  embeddingPending = $state(0);
  /** The stored documents the staged files are probably versions of, once the comparison answered. Null while it runs, and for a server that cannot compare. */
  similar = $state.raw<KnowledgeSimilar | null>(null);
  /** The comparison of the staged files is running. */
  scanning = $state(false);
  /** What the user set per matched file, which updates its document or is added as new. A file not in it takes its default (see `updatesByDefault`). */
  choices = $state.raw<MatchChoices>({});
  /** Where the import files its documents, which the dialog keeps current from the routing. A match in another place is added as new unless the user sets it to update. */
  landing = $state<Landing>('unknown');
  /** The staged files with what each matches, once compared. */
  readonly rows = $derived<MatchRow[] | null>(this.intake !== null && this.similar !== null ? matchRows(this.intake, this.similar) : null);

  private scanned = '';
  private scans = 0;

  constructor(
    private readonly transport: Transport,
    private readonly now: () => number = Date.now,
  ) {}

  get running(): boolean {
    return this.phase === 'running';
  }

  /** Opens the dialog on what is staged, or on the picking step. */
  begin(): void {
    if (this.phase === 'done') {
      this.clear();
    }

    this.open = true;
  }

  /** Closes the window. A running import goes on, a finished one is forgotten. */
  close(): void {
    this.open = false;

    if (this.phase === 'done') {
      this.clear();
    }
  }

  private clear(): void {
    this.phase = 'idle';
    this.intake = null;
    this.run = emptyRun(0);
    this.cancelling = false;
    this.forgetScan();
  }

  /** Lets the user choose files or a folder in the native dialog. A cancelled dialog changes nothing. */
  async pick(what: 'files' | 'folder'): Promise<void> {
    if (this.picking || this.running) {
      return;
    }

    this.picking = true;

    try {
      this.stage(await this.transport.invoke(what === 'files' ? 'knowledge_pick_files' : 'knowledge_pick_folder', undefined));
    } catch (error) {
      toasts.push({ title: t('knowledge.import.toast.pickFailed'), description: writeFailureText(error), tone: 'danger' });
    } finally {
      this.picking = false;
    }
  }

  /** Takes the files dropped on the window: they are staged and the dialog opens on them. A drop while an import runs is ignored. */
  async takeDrop(): Promise<void> {
    if (this.running) {
      return;
    }

    try {
      const intake = await this.transport.invoke('knowledge_drop_intake', undefined);

      if (intake !== null) {
        if (this.phase === 'done') {
          this.clear();
        }

        this.stage(intake);
        this.open = true;
      }
    } catch (error) {
      toasts.push({ title: t('knowledge.import.toast.pickFailed'), description: writeFailureText(error), tone: 'danger' });
    }
  }

  private stage(intake: KnowledgeIntake | null): void {
    if (intake === null) {
      return;
    }

    this.intake = intake;
    this.phase = intake.files.length + intake.folders.length > 0 ? 'staged' : 'idle';
    this.forgetScan();
  }

  private forgetScan(): void {
    this.scans += 1;
    this.scanned = '';
    this.similar = null;
    this.scanning = false;
    this.choices = {};
  }

  /**
   * Compares the staged files with the stored documents, so each can update the document it is a version of. A title only matches inside the scope of the first
   * link, so a change of it compares again. A comparison that fails leaves the plain list of files: the import then adds them as it always did.
   */
  async scan(): Promise<void> {
    const intake = this.intake;

    if (this.phase !== 'staged' || intake === null) {
      return;
    }

    const key = `${intake.token}|${this.links[0]?.repo ?? ''}`;

    if (key === this.scanned) {
      return;
    }

    this.scanned = key;
    this.scans += 1;
    const mine = this.scans;
    this.scanning = true;
    this.similar = null;

    try {
      const answer = await this.transport.invoke('knowledge_similar', { token: intake.token, links: this.links });

      if (mine === this.scans) {
        this.similar = answer;
      }
    } catch {
      // The files are listed without a match, and an import adds them.
    } finally {
      if (mine === this.scans) {
        this.scanning = false;
      }
    }
  }

  /** Sets whether a file that matches a document updates it or is added as new. The choice is for that match only. */
  setUpdate(path: string, update: boolean): void {
    const row = this.rows?.find((candidate) => candidate.path === path);
    const key = row === undefined ? null : choiceKey(row);

    if (key === null) {
      return;
    }

    this.choices = { ...this.choices, [key]: update };
  }

  /** Imports what is staged. The events fold into `run` as they arrive, and the answer settles what the events did not. */
  async start(): Promise<void> {
    const intake = this.intake;

    if (this.phase !== 'staged' || intake === null) {
      return;
    }

    const replacing = replacementsOf(this.rows ?? [], this.choices, this.landing);

    this.phase = 'running';
    this.cancelling = false;
    this.run = emptyRun(this.now());

    try {
      const result = await this.transport.invoke('knowledge_import', {
        token: intake.token,
        options: { links: this.links, replaces: null, ...(replacing.length === 0 ? {} : { replacing }), ...(this.place === null ? {} : { place: this.place }) },
        onEvent: (event) => {
          this.run = reduceImport(this.run, event);
        },
      });

      // A server that ended without its closing event still tells how it ended.
      if (this.run.counts === null && result.counts !== null) {
        this.run = { ...this.run, counts: result.counts, cancelled: result.cancelled };
      }
    } catch (error) {
      this.run = { ...this.run, failure: this.run.failure ?? writeFailureText(error) };
    }

    this.phase = 'done';
    this.cancelling = false;
    this.embeddingPending = this.run.embeddingPending;

    // The window was closed meanwhile: the toast is how the user hears of the end.
    if (!this.open) {
      this.tell();
    }
  }

  private tell(): void {
    const { counts, failure } = this.run;

    if (failure !== null) {
      toasts.push({ title: t('knowledge.import.toast.failed'), description: failure, tone: 'danger' });
    } else if (counts !== null) {
      const written = counts.added + counts.updated;

      toasts.push({ title: t(this.run.cancelled ? 'knowledge.import.toast.cancelled' : 'knowledge.import.toast.done'), description: t('knowledge.import.toast.counts', { written, failed: counts.failed }), tone: counts.failed > 0 ? 'warning' : 'success' });
    }
  }

  /** Asks the running import to stop after the file in hand. */
  async cancel(): Promise<void> {
    if (!this.running || this.cancelling) {
      return;
    }

    this.cancelling = true;

    try {
      await this.transport.invoke('knowledge_import_cancel', undefined);
    } catch {
      this.cancelling = false;
    }
  }

  /** Embeds the chunks that have no vector yet; the notice goes once it ran. */
  async embedNow(): Promise<void> {
    try {
      await this.transport.invoke('knowledge_reindex', { ids: null });
      this.embeddingPending = 0;
    } catch (error) {
      toasts.push({ title: t('knowledge.import.toast.embedFailed'), description: writeFailureText(error), tone: 'danger' });
    }
  }

  dismissEmbedding(): void {
    this.embeddingPending = 0;
  }

  /** Forgets everything, e.g. for a test. */
  reset(): void {
    this.open = false;
    this.clear();
    this.links = [];
    this.place = null;
    this.landing = 'unknown';
    this.picking = false;
    this.embeddingPending = 0;
  }
}

export const knowledgeImport = new KnowledgeImportStore(ipc);
