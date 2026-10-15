import { t } from '$lib/i18n/i18n.svelte';
import type { SyncExportItem } from '$lib/ipc/bindings/SyncExportItem';
import type { SyncExportPreview } from '$lib/ipc/bindings/SyncExportPreview';
import type { SyncExported } from '$lib/ipc/bindings/SyncExported';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { toasts } from '$lib/stores/toasts.svelte';

import { IdSelection } from './id-selection.svelte';

export type ExportLoad = 'idle' | 'loading' | 'ready' | 'error';

/** How long the copy button says "Copied". */
const COPIED_MS = 1600;

/**
 * The export dialog. Opening it only reads this machine; the webview sends back item ids, never a bundle: Rust
 * rebuilds the share code from disk for exactly the ticked ids.
 */
export class ExportController {
  load = $state<ExportLoad>('idle');
  preview = $state.raw<SyncExportPreview | null>(null);
  error = $state<string | null>(null);

  /** The code of the current selection; any change of the selection forgets it. */
  code = $state.raw<SyncExported | null>(null);
  copied = $state(false);
  /** The path of the file saved for the current selection. */
  savedTo = $state<string | null>(null);
  /** The save dialog was closed without a file. */
  saveCancelled = $state(false);
  busy = $state(false);
  failure = $state<string | null>(null);

  private picks = new IdSelection();
  private token = 0;
  /** Counts every change of the selection, so a result that arrives for an older one can be told apart. */
  private selection = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly transport: Transport) {}

  /** Reads the machine and ticks everything. */
  async open(): Promise<void> {
    this.token += 1;
    this.selection += 1;
    const mine = this.token;

    this.load = 'loading';
    this.error = null;
    this.forget();
    this.failure = null;

    try {
      const preview = await this.transport.invoke('sync_export_preview', undefined);

      if (mine !== this.token) {
        return;
      }

      this.preview = preview;
      this.picks.replace(this.items.map((item) => item.id));
      this.load = 'ready';
    } catch (error) {
      if (mine === this.token) {
        this.preview = null;
        this.error = IpcFailure.from(error).message;
        this.load = 'error';
      }
    }
  }

  get items(): SyncExportItem[] {
    return this.preview?.sections.flatMap((section) => section.items) ?? [];
  }

  get empty(): boolean {
    return this.load === 'ready' && this.items.length === 0;
  }

  /** Ids in the order the list shows them. */
  get selectedIds(): string[] {
    return this.picks.inOrder(this.items);
  }

  isSelected(id: string): boolean {
    return this.picks.has(id);
  }

  /** Ticks or unticks every item of a group at once. */
  toggleMany(ids: readonly string[], on: boolean): void {
    const next = this.picks.copy();

    for (const id of ids) {
      if (on) {
        next.add(id);
      } else {
        next.delete(id);
      }
    }

    this.picks.replace(next);
    this.selection += 1;
    this.forget();
  }

  toggle(id: string, on: boolean): void {
    const next = this.picks.copy();

    if (on) {
      next.add(id);
    } else {
      next.delete(id);
    }

    this.picks.replace(next);
    this.selection += 1;
    this.forget();
  }

  /** Builds the code for the selection, puts it on the clipboard and shows it. */
  async copy(): Promise<void> {
    if (this.busy || this.selectedIds.length === 0) {
      return;
    }

    const mine = this.selection;

    this.busy = true;
    this.failure = null;

    try {
      const code = await this.transport.invoke('sync_export', { selected: this.selectedIds });

      // The ticks changed while the code was being built: it belongs to a selection that is gone.
      if (mine !== this.selection) {
        return;
      }

      this.code = code;
      await this.toClipboard(code.code);
    } catch (error) {
      if (mine === this.selection) {
        this.failure = IpcFailure.from(error).message;
      }
    } finally {
      this.busy = false;
    }
  }

  /** Opens the native save dialog; Rust writes the file for the selection. */
  async save(): Promise<void> {
    if (this.busy || this.selectedIds.length === 0) {
      return;
    }

    const mine = this.selection;

    this.busy = true;
    this.failure = null;
    this.saveCancelled = false;
    this.savedTo = null;

    try {
      const path = await this.transport.invoke('sync_export_save', { selected: this.selectedIds });

      if (mine !== this.selection) {
        return;
      }

      this.savedTo = path;
      this.saveCancelled = path === null;
    } catch (error) {
      if (mine === this.selection) {
        this.failure = IpcFailure.from(error).message;
      }
    } finally {
      this.busy = false;
    }
  }

  private async toClipboard(text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      toasts.push({ title: t('components.copy.failed'), description: t('components.copy.unavailable'), tone: 'warning' });

      return;
    }

    this.copied = true;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => (this.copied = false), COPIED_MS);
  }

  private forget(): void {
    this.code = null;
    this.copied = false;
    this.savedTo = null;
    this.saveCancelled = false;
  }
}
