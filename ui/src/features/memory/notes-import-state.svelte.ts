import { t } from '$lib/i18n/i18n.svelte';
import type { NotesImportFormat } from '$lib/ipc/bindings/NotesImportFormat';
import type { NotesImportPlan } from '$lib/ipc/bindings/NotesImportPlan';
import type { NotesImportProgress } from '$lib/ipc/bindings/NotesImportProgress';
import type { NotesImportResult } from '$lib/ipc/bindings/NotesImportResult';
import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { allPicks, defaultPicks, type NotesImportRefusal, pickedIndexes } from './notes-import';

/** What the dialog shows: the format a file needs, the review of a file, why a file was refused, the run, or what the run did. */
export type ImportStage = 'start' | 'review' | 'refused' | 'applying' | 'result';

/**
 * The notes import. The Notes menu and the empty list both open the dialog on the format a file needs, and its button opens the native
 * picker: Rust reads the file into a plan, and the dialog turns to what there is to say about the file. Importing sends the plan's id and the ticked rows, never the notes.
 * The plan is good for one import, so after it (or a failed one) another file has to be chosen.
 */
class NotesImport {
  open = $state(false);
  stage = $state<ImportStage>('start');
  /** The native picker is open, or the file is being read. */
  picking = $state(false);
  /** The native save dialog is open, or the example file is being written. */
  savingTemplate = $state(false);

  plan = $state.raw<NotesImportPlan | null>(null);
  refusal = $state.raw<NotesImportRefusal | null>(null);
  /** The rows of the plan that will be imported. */
  picks = $state.raw<Set<number>>(new Set());
  /** The scope of the rows whose file names none. */
  defaultScope = $state<ScriptScope>('global');
  /** Rewrite the repository note that has a row's title, instead of skipping the row. */
  replace = $state(false);

  progress = $state<NotesImportProgress | null>(null);
  result = $state.raw<NotesImportResult | null>(null);
  /** Why the import could not run at all. */
  failure = $state<string | null>(null);

  /** The project the plan was made for. */
  private project = $state<string | null>(null);

  /** Raised by `reset`, so a file or an import that was under way before it is dropped instead of opening the dialog again. */
  private generation = 0;

  /** The project the file was reviewed for: the only repository the rows can be kept in, since the plan was read for it. */
  get reviewedFor(): string | null {
    return this.project;
  }

  get rows() {
    return this.plan?.rows ?? [];
  }

  get picked(): number[] {
    return pickedIndexes(this.rows, this.picks);
  }

  /** Opens the dialog on the format a file needs, whatever was reviewed before. */
  begin(): void {
    if (this.picking || this.stage === 'applying') {
      return;
    }

    this.plan = null;
    this.refusal = null;
    this.failure = null;
    this.result = null;
    this.progress = null;
    this.stage = 'start';
    this.open = true;
  }

  /** Saves an example file of notes in `format`, where the user chooses, and says where it went. */
  async saveTemplate(format: NotesImportFormat): Promise<void> {
    if (this.savingTemplate) {
      return;
    }

    this.savingTemplate = true;

    try {
      const saved = await ipc.invoke('memory_notes_import_template', { format });

      if (saved !== null) {
        toasts.push({ title: t('memory.import.template.saved'), description: saved, tone: 'success' });
      }
    } catch (error) {
      toasts.push({ title: t('memory.import.template.failed'), description: IpcFailure.from(error).message, tone: 'warning' });
    } finally {
      this.savingTemplate = false;
    }
  }

  /** Opens the picker for the notes of `project`. Cancelling it leaves everything as it was. */
  async choose(project: string | null): Promise<void> {
    if (this.picking || this.stage === 'applying') {
      return;
    }

    const generation = this.generation;
    this.picking = true;

    try {
      const preview = await ipc.invoke('memory_notes_import_preview', { project });

      if (preview === null || generation !== this.generation) {
        return;
      }

      this.project = project;
      this.failure = null;
      this.result = null;
      this.progress = null;

      if (preview.kind === 'refused') {
        this.plan = null;
        this.refusal = preview;
        this.stage = 'refused';
      } else {
        this.plan = preview.plan;
        this.refusal = null;
        this.picks = defaultPicks(preview.plan.rows);
        this.defaultScope = project === null ? 'global' : 'repo';
        this.replace = false;
        this.stage = 'review';
      }

      this.open = true;
    } catch (error) {
      if (generation === this.generation) {
        toasts.push({ title: t('memory.import.failed.read'), description: IpcFailure.from(error).message, tone: 'danger' });
      }
    } finally {
      if (generation === this.generation) {
        this.picking = false;
      }
    }
  }

  toggle(index: number, on: boolean): void {
    const next = new Set(this.picks);

    if (on) {
      next.add(index);
    } else {
      next.delete(index);
    }

    this.picks = next;
  }

  toggleAll(on: boolean): void {
    this.picks = allPicks(this.rows, on);
  }

  /** Imports the ticked rows, one at a time in the file's order. */
  async apply(): Promise<void> {
    const plan = this.plan;
    const rows = this.picked;

    if (!plan || this.stage !== 'review' || rows.length === 0) {
      return;
    }

    const generation = this.generation;
    this.stage = 'applying';
    this.progress = { index: rows[0], done: 0, total: rows.length };
    this.failure = null;

    try {
      const result = await ipc.invoke('memory_notes_import_apply', {
        planId: plan.planId,
        rows,
        defaultScope: this.defaultScope,
        replace: this.replace,
        project: this.project,
        onProgress: (event) => {
          if (generation === this.generation) {
            this.progress = event;
          }
        },
      });

      if (generation === this.generation) {
        this.result = result;
      }
    } catch (error) {
      if (generation === this.generation) {
        this.failure = IpcFailure.from(error).message;
      }
    } finally {
      if (generation === this.generation) {
        this.stage = 'result';
      }
    }

    // Notes saved before a failure stay saved, so the list is read again either way.
    await noteList.refresh({ quiet: true });
  }

  /** Forgets the file, e.g. when the project it was reviewed for is no longer the one on screen. */
  reset(): void {
    this.generation += 1;
    this.open = false;
    this.stage = 'start';
    this.picking = false;
    this.savingTemplate = false;
    this.plan = null;
    this.refusal = null;
    this.picks = new Set();
    this.replace = false;
    this.progress = null;
    this.result = null;
    this.failure = null;
  }
}

export const notesImport = new NotesImport();
