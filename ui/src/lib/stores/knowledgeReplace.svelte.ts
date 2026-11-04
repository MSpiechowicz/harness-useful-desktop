import { t } from '$lib/i18n/i18n.svelte';
import type { DropSpot } from '$lib/ipc/bindings/DropSpot';
import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgeIntake } from '$lib/ipc/bindings/KnowledgeIntake';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';
import { writeFailureText } from '$lib/utils/knowledge-failure';

import { emptyRun, reduceImport } from './knowledgeImport.svelte';
import { toasts } from './toasts.svelte';

/** The one file chosen to replace a document. The shell keeps its path: the import takes the token and nothing else names a path. */
export interface ReplaceFile {
  token: string;
  name: string;
  bytes: number | null;
}

/** Where the panel of the open document is on the page, and what a file dropped on it does. */
export interface ReplaceTarget {
  rect: () => Pick<DOMRect, 'left' | 'top' | 'right' | 'bottom'> | null;
  drop: () => void;
}

/** Where a native drag goes: one file over the panel replaces its document, anything else (many files, a folder, elsewhere on the page) is an import. */
export function routeNativeDrop(at: DropSpot | undefined, target: ReplaceTarget | null): 'replace' | 'import' {
  const rect = target?.rect() ?? null;

  if (at === undefined || rect === null || at.items !== 1 || at.folder) {
    return 'import';
  }

  const inside = at.x >= rect.left && at.x < rect.right && at.y >= rect.top && at.y < rect.bottom;

  return inside ? 'replace' : 'import';
}

/**
 * Replaces one document by a file, in place: the file is picked (or dropped) through the intake like any import, and the import names the document it replaces.
 * It does not touch the import dialog or its state, so a replace can run while the dialog is closed.
 */
export class KnowledgeReplaceStore {
  /** A native dialog is open. */
  picking = $state(false);
  /** The replacing import is running. */
  busy = $state(false);
  /** A file is dragged over the panel of the open document, which then takes the drop instead of the window. */
  overPanel = $state(false);
  /** The open document's panel while it takes a dropped file. A native drag names no element, so it is told where the panel is. */
  target = $state.raw<ReplaceTarget | null>(null);

  constructor(private readonly transport: Transport) {}

  /** Follows a native drag (the shell names where it is): the panel says what a drop on it does. */
  nativeHover(active: boolean, at: DropSpot | undefined): void {
    this.overPanel = active && routeNativeDrop(at, this.target) === 'replace';
  }

  /** A native drop: starts the replace when it landed on the panel and answers true, otherwise false and the drop is for the import. */
  nativeDrop(at: DropSpot | undefined): boolean {
    const target = this.target;

    this.overPanel = false;

    if (target === null || routeNativeDrop(at, target) !== 'replace') {
      return false;
    }

    target.drop();

    return true;
  }

  /** Lets the user choose the file in the native dialog. Null when the dialog is cancelled, or when more than one file or a folder was chosen (with a toast). */
  async pick(): Promise<ReplaceFile | null> {
    if (this.picking || this.busy) {
      return null;
    }

    this.picking = true;

    try {
      return this.single(await this.transport.invoke('knowledge_pick_files', { multiple: false }));
    } catch (error) {
      toasts.push({ title: t('knowledge.import.toast.pickFailed'), description: writeFailureText(error), tone: 'danger' });

      return null;
    } finally {
      this.picking = false;
    }
  }

  /** The file dropped on the panel, as `pick` answers. */
  async takeDrop(): Promise<ReplaceFile | null> {
    if (this.busy) {
      return null;
    }

    try {
      return this.single(await this.transport.invoke('knowledge_drop_intake', undefined));
    } catch (error) {
      toasts.push({ title: t('knowledge.import.toast.pickFailed'), description: writeFailureText(error), tone: 'danger' });

      return null;
    }
  }

  private single(intake: KnowledgeIntake | null): ReplaceFile | null {
    if (intake === null || intake.files.length + intake.folders.length === 0) {
      return null;
    }

    const [file] = intake.files;

    if (intake.files.length !== 1 || intake.folders.length > 0 || !file) {
      toasts.push({ title: t('knowledge.replace.pickOne'), description: t('knowledge.replace.pickOne.body'), tone: 'warning' });

      return null;
    }

    return { token: intake.token, name: file.name, bytes: file.bytes };
  }

  /**
   * Replaces `doc` by `file`, keeping where it applies. With `ask` (and an optional `note` for the reviewers) the replacement is also asked for approval in one step.
   * Says how it went in a toast, and answers whether the document now holds the file's text.
   */
  async replace(doc: Pick<KnowledgeDocument, 'id' | 'links' | 'title'>, file: ReplaceFile, ask: { note: string } | null = null): Promise<boolean> {
    if (this.busy) {
      return false;
    }

    this.busy = true;

    let run = emptyRun(0);

    try {
      await this.transport.invoke('knowledge_import', {
        token: file.token,
        options: { links: doc.links, replaces: doc.id, ...(ask ? { ask: true, ...(ask.note.trim() ? { note: ask.note.trim() } : {}) } : {}) },
        onEvent: (event) => {
          run = reduceImport(run, event);
        },
      });
    } catch (error) {
      run = { ...run, failure: run.failure ?? writeFailureText(error) };
    } finally {
      this.busy = false;
    }

    return this.tell(doc.title, run, ask !== null);
  }

  private tell(title: string, run: ReturnType<typeof emptyRun>, asked: boolean): boolean {
    const [row] = run.rows;

    if (run.failure !== null || row === undefined || row.status === 'failed' || row.status === 'skipped') {
      toasts.push({ title: t('knowledge.toast.replaceFailed'), description: run.failure || row?.reason || title, tone: 'danger' });

      return false;
    }

    // A file with the text the document has changes nothing, unless it was sent for approval in the same step: that ask was made, so it says so.
    if (row.status === 'unchanged' && !row.asked) {
      toasts.push({ title: t('knowledge.toast.replaceUnchanged'), description: t('knowledge.toast.replaceUnchanged.body'), tone: 'info' });

      return false;
    }

    // With team approval on a replaced live document becomes a pending update, which agents use once a teammate approves it.
    const key = row.status === 'pending' ? 'knowledge.drafts.toast.replacedPending' : asked ? 'knowledge.drafts.toast.replacedAsked' : 'knowledge.toast.replaced';

    toasts.push({ title: t(key), description: title, tone: 'success' });

    return true;
  }

  /** Forgets everything, e.g. for a test. */
  reset(): void {
    this.picking = false;
    this.busy = false;
    this.overPanel = false;
    this.target = null;
  }
}

export const knowledgeReplace = new KnowledgeReplaceStore(ipc);
