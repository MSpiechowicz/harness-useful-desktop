import { t } from '$lib/i18n/i18n.svelte';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { SyncImportItem } from '$lib/ipc/bindings/SyncImportItem';
import type { SyncImportPlan } from '$lib/ipc/bindings/SyncImportPlan';
import type { SyncImportResult } from '$lib/ipc/bindings/SyncImportResult';
import type { SyncItemKind } from '$lib/ipc/bindings/SyncItemKind';
import type { SyncItemOutcome } from '$lib/ipc/bindings/SyncItemOutcome';
import type { SyncNote } from '$lib/ipc/bindings/SyncNote';
import type { SyncProgress } from '$lib/ipc/bindings/SyncProgress';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { IdSelection } from './id-selection.svelte';
import { failedCliNote, itemLabel, manualEditOf, outcomeChip, outcomeNote, type Chip } from './sync-model';

export type ImportStage = 'input' | 'review' | 'applying' | 'result';

/** One line of the applying and result screens. */
export interface RunRow {
  id: string;
  harness: HarnessId;
  kind: SyncItemKind;
  label: string;
  chip: Chip;
  note: string | null;
  /** The remarks on the outcome, such as an unreadable config file behind a conflict. */
  notes: SyncNote[];
  /** The redacted outcome line of the command that ran. */
  summary: string | null;
  /** The command line that ran. */
  argv: string[];
  /** The entry to add by hand to a file the app left alone, when that is why the item was skipped. */
  manualEdit: { file: string; snippet: string } | null;
}

/**
 * The import dialog. Pasting or opening a file only builds a reviewed plan; nothing runs until the user has seen the
 * exact steps, ticked what they want and confirmed. Applying sends the plan id and the ticked ids, never the bundle.
 */
export class ImportController {
  stage = $state<ImportStage>('input');
  text = $state('');
  /** Reading the input into a plan. */
  reviewing = $state(false);
  /** The backend's message when the input was refused. */
  error = $state<string | null>(null);
  /** The file picker was closed without a file. */
  noFile = $state(false);

  plan = $state.raw<SyncImportPlan | null>(null);
  result = $state.raw<SyncImportResult | null>(null);
  /** Why the import could not run at all. */
  failure = $state<string | null>(null);

  private picks = new IdSelection();
  /** Counts the sessions (every reset starts one), so an answer to a question of a closed session is ignored. */
  private session = 0;
  /** Dependents that were unticked together with their marketplace, so ticking it again restores them. */
  private lifted = $state.raw(new Map<string, string[]>());
  private running = $state<string | null>(null);
  private finished = $state.raw<Record<string, SyncItemOutcome>>({});

  constructor(private readonly transport: Transport) {}

  /** Back to an empty input. */
  reset(): void {
    this.session += 1;
    this.stage = 'input';
    this.reviewing = false;
    this.text = '';
    this.reviewing = false;
    this.error = null;
    this.noFile = false;
    this.plan = null;
    this.result = null;
    this.failure = null;
    this.picks.replace([]);
    this.lifted = new Map();
    this.running = null;
    this.finished = {};
  }

  // ── Input ────────────────────────────────────────────────────────────────

  /** Reads the pasted text into a plan. Only the Review button calls this; typing or pasting never does. */
  async review(): Promise<void> {
    if (this.text.trim() !== '') {
      await this.preview({ type: 'text', text: this.text });
    }
  }

  /** Lets Rust open the native picker and read the file into a plan. */
  async openFile(): Promise<void> {
    await this.preview({ type: 'file' });
  }

  private async preview(input: { type: 'text'; text: string } | { type: 'file' }): Promise<void> {
    if (this.reviewing) {
      return;
    }

    const mine = this.session;

    this.reviewing = true;
    this.error = null;
    this.noFile = false;

    try {
      const plan = await this.transport.invoke('sync_import_preview', { input });

      if (mine !== this.session) {
        return;
      }

      if (plan === null) {
        this.noFile = true;
      } else {
        this.show(plan);
      }
    } catch (error) {
      if (mine === this.session) {
        this.error = IpcFailure.from(error).message;
      }
    } finally {
      if (mine === this.session) {
        this.reviewing = false;
      }
    }
  }

  private show(plan: SyncImportPlan): void {
    this.plan = plan;
    this.lifted = new Map();

    const ticked = new Set(this.items.filter((item) => item.selectable && item.selectedByDefault).map((item) => item.id));

    this.picks.replace(this.items.filter((item) => ticked.has(item.id) && !this.waitsFor(item, (id) => ticked.has(id))).map((item) => item.id));
    this.stage = 'review';
  }

  back(): void {
    this.stage = 'input';
    this.plan = null;
  }

  // ── Selection ────────────────────────────────────────────────────────────

  /** Every item of the plan, in the order it is applied. */
  get items(): SyncImportItem[] {
    return this.plan?.sections.flatMap((section) => section.items) ?? [];
  }

  get total(): number {
    return this.items.filter((item) => item.selectable).length;
  }

  get selectedIds(): string[] {
    return this.picks.inOrder(this.items);
  }

  isSelected(id: string): boolean {
    return this.picks.has(id);
  }

  /** The item needs a marketplace of this setup that is not going to be imported. */
  private waitsFor(item: SyncImportItem, picked: (id: string) => boolean): boolean {
    if (item.dependsOn === null) {
      return false;
    }

    const parent = this.items.find((other) => other.id === item.dependsOn);

    return !!parent?.selectable && !picked(parent.id);
  }

  /** The checkbox cannot be ticked: the item is not selectable, or its marketplace is unticked. */
  locked(item: SyncImportItem): boolean {
    return !item.selectable || this.waitsFor(item, (id) => this.picks.has(id));
  }

  /** Ticking a marketplace again restores the plugins that were unticked with it. */
  toggle(id: string, on: boolean): void {
    const item = this.items.find((candidate) => candidate.id === id);

    if (!item || this.locked(item)) {
      return;
    }

    const next = this.picks.copy();
    const lifted = new Map(this.lifted);

    if (on) {
      next.add(id);
      lifted.get(id)?.forEach((dependent) => next.add(dependent));
      lifted.delete(id);
    } else {
      next.delete(id);

      const dependents = this.items.filter((other) => other.dependsOn === id && next.has(other.id)).map((other) => other.id);
      dependents.forEach((dependent) => next.delete(dependent));

      if (dependents.length > 0) {
        lifted.set(id, dependents);
      }
    }

    this.picks.replace(next);
    this.lifted = lifted;
  }

  /** Ticks or unticks every item of a group that can be chosen, a marketplace before its plugins. */
  toggleMany(items: readonly SyncImportItem[], on: boolean): void {
    for (const item of items) {
      if (item.selectable) {
        this.toggle(item.id, on);
      }
    }
  }

  // ── Applying ─────────────────────────────────────────────────────────────

  /** Runs the confirmed selection, one item at a time in the order of the plan. */
  async apply(): Promise<void> {
    const plan = this.plan;
    const selected = this.selectedIds;

    if (!plan || this.stage !== 'review' || selected.length === 0) {
      return;
    }

    this.stage = 'applying';
    this.failure = null;
    this.finished = {};
    this.running = null;

    try {
      this.result = await this.transport.invoke('sync_import_apply', {
        planId: plan.planId,
        selected,
        confirmed: true,
        onProgress: (event) => this.progress(event),
      });
    } catch (error) {
      this.failure = IpcFailure.from(error).message;
    } finally {
      this.running = null;
      this.stage = 'result';
    }
  }

  private progress(event: SyncProgress): void {
    if (event.type === 'started') {
      this.running = event.id;
    } else {
      this.finished = { ...this.finished, [event.outcome.id]: event.outcome };
      this.running = null;
    }
  }

  /** What the run screens list: the final outcomes, or the selected items with what they have done so far. */
  get runRows(): RunRow[] {
    if (this.result) {
      const labels = new Map((this.plan?.sections ?? []).flatMap((section) => section.items.map((item) => [item.id, itemLabel(section.harness, item)] as const)));

      return this.result.items.map((outcome) => finishedRow(outcome, labels.get(outcome.id)));
    }

    const rows: RunRow[] = [];

    for (const section of this.plan?.sections ?? []) {
      for (const item of section.items.filter((candidate) => this.picks.has(candidate.id))) {
        const done = this.finished[item.id];

        rows.push(done ? finishedRow(done, itemLabel(section.harness, item)) : this.pendingRow(section.harness, item));
      }
    }

    return rows;
  }

  /** An item that has not finished: waiting for its turn, or running now. */
  private pendingRow(harness: HarnessId, item: SyncImportItem): RunRow {
    const chip: Chip = this.running === item.id ? { tone: 'accent', label: t('sync.run.running'), busy: true } : { tone: 'neutral', label: t('sync.run.queued') };

    return { id: item.id, harness, kind: item.kind, label: itemLabel(harness, item), chip, note: null, notes: [], summary: null, argv: [], manualEdit: null };
  }
}

/** A finished item, listed by `label` (the plan's name for it) or else the name the run reported. */
function finishedRow(outcome: SyncItemOutcome, label: string = outcome.label): RunRow {
  return {
    id: outcome.id,
    harness: outcome.harness,
    kind: outcome.kind,
    label,
    chip: outcomeChip(outcome.outcome),
    note: failedCliNote(outcome) ?? outcomeNote(outcome.outcome, outcome.notes),
    notes: outcome.notes,
    summary: outcome.summary,
    argv: outcome.argv,
    manualEdit: manualEditOf(outcome.outcome),
  };
}
