export type ToastTone = 'neutral' | 'success' | 'warning' | 'danger' | 'info';

export interface ToastInput {
  title: string;
  description?: string;
  tone?: ToastTone;
  /** Milliseconds before auto-dismiss; 0 keeps the toast until dismissed. */
  duration?: number;
  /** One follow-up action, e.g. "Reveal"; running it dismisses the toast. */
  action?: ToastAction;
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
    };

    this.items = [...this.items, item].slice(-MAX_VISIBLE);

    return id;
  }

  dismiss(id: number): void {
    this.items = this.items.filter((item) => item.id !== id);
  }
}

export const toasts = new ToastsStore();
