export type ToastTone = 'neutral' | 'success' | 'warning' | 'danger' | 'info';

export interface ToastInput {
  title: string;
  description?: string;
  tone?: ToastTone;
  /** Milliseconds before auto-dismiss; 0 keeps the toast until dismissed. */
  duration?: number;
  /** One follow-up action, e.g. "Reveal"; running it dismisses the toast. */
  action?: ToastAction;
  /** The project folder the toast is about: it is dismissed when another project becomes the active one. */
  project?: string;
}

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface ToastItem {
  id: number;
  title: string;
  description?: string;
  tone: ToastTone;
  duration: number;
  action?: ToastAction;
  project?: string;
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
    };

    this.items = [...this.items, item].slice(-MAX_VISIBLE);

    return id;
  }

  /**
   * Changes a toast that is still shown, e.g. the progress of a long task. A key left out stays as it was, `description`, `action` and
   * `project` can be cleared with `undefined`. A toast that is gone stays gone. Auto-dismiss starts over when `duration` changes.
   */
  update(id: number, patch: Partial<ToastInput>): void {
    const item = this.items.find((each) => each.id === id);

    if (!item) {
      return;
    }

    if (patch.title !== undefined) {
      item.title = patch.title;
    }

    if ('description' in patch) {
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
  }

  dismiss(id: number): void {
    this.items = this.items.filter((item) => item.id !== id);
  }

  /** Dismisses the toasts about a project other than `path`, the one that became active (see `ToastInput.project`). */
  leaveProjects(path: string | null): void {
    this.items = this.items.filter((item) => item.project === undefined || item.project === path);
  }
}

export const toasts = new ToastsStore();
