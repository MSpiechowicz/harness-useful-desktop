export type ToastTone = 'neutral' | 'success' | 'warning' | 'danger' | 'info';

export interface ToastInput {
  /** A short, small heading. */
  title: string;
  /** The line under the title that says what happened or what to do next: every toast has one. */
  description: string;
  tone?: ToastTone;
  /** Milliseconds before auto-dismiss; 0 keeps the toast until dismissed. */
  duration?: number;
  /** One follow-up action, e.g. "Reveal"; running it dismisses the toast. */
  action?: ToastAction;
  /** The project folder the toast is about: it is dismissed when another project becomes the active one. */
  project?: string;
  /** Called once when the toast is dismissed: its time ran out, the close button was pressed or its action ran. A toast pushed out of the visible stack does not call it. */
  onclose?: () => void;
}

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface ToastItem {
  id: number;
  title: string;
  description: string;
  tone: ToastTone;
  duration: number;
  action?: ToastAction;
  project?: string;
  onclose?: () => void;
  /** How many times the toast was updated: the auto-dismiss countdown starts over with each one. */
  revision?: number;
}

const DEFAULT_DURATION_MS = 5000;
const MAX_VISIBLE = 5;

export class ToastsStore {
  items = $state<ToastItem[]>([]);

  private nextId = 1;

  push(input: ToastInput): number {
    const id = this.nextId;
    this.nextId += 1;

    const item: ToastItem = {
      id,
      title: input.title,
      description: input.description,
      tone: input.tone ?? 'neutral',
      duration: input.duration ?? DEFAULT_DURATION_MS,
      action: input.action,
      project: input.project,
      onclose: input.onclose,
      revision: 0,
    };

    this.items = [...this.items, item].slice(-MAX_VISIBLE);

    return id;
  }

  /**
   * Changes a toast that is still shown, e.g. the progress of a long task. A key left out stays as it was, `action` and `project` can be
   * cleared with `undefined`. A toast that is gone stays gone. Auto-dismiss starts over with every update, so one toast
   * can report the same thing again and again without stacking.
   */
  update(id: number, patch: Partial<ToastInput>): void {
    const item = this.items.find((each) => each.id === id);

    if (!item) {
      return;
    }

    if (patch.title !== undefined) {
      item.title = patch.title;
    }

    if (patch.description !== undefined) {
      item.description = patch.description;
    }

    if (patch.tone !== undefined) {
      item.tone = patch.tone;
    }

    if (patch.duration !== undefined) {
      item.duration = patch.duration;
    }

    if ('action' in patch) {
      item.action = patch.action;
    }

    if ('project' in patch) {
      item.project = patch.project;
    }

    item.revision = (item.revision ?? 0) + 1;
  }

  /** Whether the toast is still shown. */
  has(id: number): boolean {
    return this.items.some((each) => each.id === id);
  }

  dismiss(id: number): void {
    const item = this.items.find((each) => each.id === id);

    if (!item) {
      return;
    }

    this.items = this.items.filter((each) => each.id !== id);
    item.onclose?.();
  }

  /** Dismisses the toasts about a project other than `path`, the one that became active (see `ToastInput.project`). */
  leaveProjects(path: string | null): void {
    this.items = this.items.filter((item) => item.project === undefined || item.project === path);
  }
}

export const toasts = new ToastsStore();
