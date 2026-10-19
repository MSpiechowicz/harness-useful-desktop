import { t, tn } from '$lib/i18n/i18n.svelte';
import type { MemoryScriptsPage } from '$lib/ipc/bindings/MemoryScriptsPage';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import type { ScriptDraft } from '$lib/ipc/bindings/ScriptDraft';
import type { ScriptGroup } from '$lib/ipc/bindings/ScriptGroup';
import type { ScriptReview } from '$lib/ipc/bindings/ScriptReview';
import type { ScriptSaved } from '$lib/ipc/bindings/ScriptSaved';
import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';
import type { ScriptSort } from '$lib/ipc/bindings/ScriptSort';
import type { SortOrder } from '$lib/ipc/bindings/SortOrder';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';

import { toasts } from './toasts.svelte';
import { viewState } from './view-state.svelte';
import { ViewLoader } from './view-loader.svelte';

/** How many scripts one page of the table holds. */
export const SCRIPTS_PAGE_SIZE = 50;

/**
 * What the table asks the server for: one page of the scripts that answer the search and the filters, in this order.
 * A server that combines filters is asked for `scope` and `review`, an older one for the one `group`: never both.
 */
export interface ScriptsRequest {
  query: string;
  scope: ScriptScope | null;
  review: ScriptReview | null;
  group: ScriptGroup | null;
  sort: ScriptSort;
  order: SortOrder;
  page: number;
}

/** The request as the loader's key: the loader fetches again whenever any of it changes, and drops answers to older ones. */
export function scriptsKey(request: ScriptsRequest): string {
  return JSON.stringify(request);
}

/** One page of the scripts agents saved in Harness Memory that this project sees (its own and the global ones), with the size of every group, scope and approval. */
export const scriptList = new ViewLoader<MemoryScriptsPage, string>((key, project) => {
  const { query, scope, review, group, sort, order, page } = JSON.parse(key) as ScriptsRequest;
  const search = query.trim();

  return ipc.invoke('memory_scripts_list', {
    project,
    offset: page * SCRIPTS_PAGE_SIZE,
    limit: SCRIPTS_PAGE_SIZE,
    query: search === '' ? undefined : search,
    scope: scope ?? undefined,
    review: review ?? undefined,
    group: group ?? undefined,
    sort,
    order,
  });
});

/** The script on screen, body included. The list carries no bodies, so this is read when one is chosen. */
/**
 * Whether a `notFound` from Harness Memory means the note or script is gone. A missing `harness-memory` executable answers
 * `notFound` too (`unavailable_error` in business/src/ecosystem/memory_scripts.rs), and an update replaces it for a moment.
 */
export function memoryItemGone(failure: IpcFailure): boolean {
  return failure.message !== 'not found: executable harness-memory';
}

export const scriptDetail = new ViewLoader<ScriptDetail, string>((id, project) => ipc.invoke('memory_script_get', { id: Number(id), project }), { goneIsFinal: memoryItemGone });

/** Deletes a script once the user confirms, then re-reads the list so the row is gone (or back, if the call failed). */
export async function deleteScript(id: number, project: string | null, confirm: () => Promise<boolean>): Promise<boolean> {
  if (!(await confirm())) {
    return false;
  }

  try {
    await ipc.invoke('memory_script_delete', { id, project });
  } catch (error) {
    toasts.push({ title: t('memory.scripts.delete.failed'), description: IpcFailure.from(error).message, tone: 'danger' });
    await scriptList.refresh();

    return false;
  }

  if (viewState.scriptId === id) {
    viewState.scriptId = null;
  }
  await scriptList.refresh();

  return true;
}

/** The most scripts one `memory_scripts_trust` call carries: the server checks each one, and the backend allows 20 s plus 1 s per script. */
const TRUST_BATCH = 50;

/** A script the user looked at, and the exact version they looked at: trusting pins that hash, so a rewrite in between is refused. */
export interface ReviewedScript {
  id: number;
  name: string;
  /** The hash of the body the user saw. */
  sha256: string;
}

/**
 * Trusts the given scripts on this computer, so agents may run them, then re-reads the list and the open script so the approval on screen is the new one.
 * Each is trusted as the version in `sha256` only. A script rewritten since is not trusted and is named, the others still are, and its new version is shown again.
 */
export async function trustScripts(scripts: ReviewedScript[], project: string | null): Promise<boolean> {
  let trusted = 0;
  const refused: ReviewedScript[] = [];
  let failure: IpcFailure | null = null;

  for (let from = 0; from < scripts.length && failure === null; from += TRUST_BATCH) {
    const batch = scripts.slice(from, from + TRUST_BATCH);

    try {
      await ipc.invoke('memory_scripts_trust', { items: batch.map(({ id, sha256 }) => ({ id, sha256 })), project });
      trusted += batch.length;
    } catch (error) {
      const cause = IpcFailure.from(error);

      // The server approved the scripts of a batch that did not change; the changed ones are found below.
      if (cause.code === 'conflict') {
        refused.push(...batch);
      } else {
        failure = cause;
      }
    }
  }

  await Promise.all([scriptList.refresh({ quiet: true }), scriptDetail.refresh({ quiet: true })]);

  const changed = changedSince(refused);

  // Only when the changed ones are known is it known how many of a refused batch the server approved.
  if (changed.length > 0) {
    trusted += refused.length - changed.length;
  }

  if (trusted > 0) {
    toasts.push({ title: tn('memory.scripts.trust.done', trusted), tone: 'success' });
  }

  for (const script of changed) {
    toasts.push({ title: t('memory.scripts.trust.changed', { name: script.name }), description: t('memory.scripts.trust.changedHint'), tone: 'warning' });
  }

  // A conflict that names no script (the list could not be read again) is still said, not swallowed.
  if (failure !== null || (refused.length > 0 && changed.length === 0)) {
    toasts.push({ title: t('memory.scripts.trust.failed'), description: failure?.message, tone: 'danger' });
  }

  return failure === null && changed.length === 0 && refused.length === 0;
}

/** The scripts whose version in the freshly read list is no longer the one the user looked at. */
function changedSince(asked: ReviewedScript[]): ReviewedScript[] {
  const now = new Map((scriptList.data?.scripts ?? []).map((script) => [script.id, script.sha256]));

  return asked.filter((script) => now.has(script.id) && now.get(script.id) !== script.sha256);
}

/** What a save of an edit found: the script changed or was deleted since the editor opened it, so nothing was written. */
export const SCRIPT_CHANGED = 'changed';

/**
 * Writes a script, then re-reads the list and the open script so what is on screen is what was saved. A script saved here is trusted on this computer.
 * Without `replace` a name that is taken is `exists` and nothing is written, with it the script of that name is overwritten (an edit, or a confirmed replace).
 * An edit names the version it opened as `base`: if the script is another one by now, nothing is written and the answer is `SCRIPT_CHANGED`, for the editor to say.
 * Any other failure is said in a toast, and answers null.
 */
export async function saveScript(project: string | null, draft: ScriptDraft, replace: boolean, base?: string): Promise<ScriptSaved | typeof SCRIPT_CHANGED | null> {
  let saved: ScriptSaved;

  try {
    saved = await ipc.invoke('memory_script_save', base === undefined ? { project, draft, replace } : { project, draft, replace, base });
  } catch (error) {
    const cause = IpcFailure.from(error);

    if (base !== undefined && cause.code === 'conflict') {
      return SCRIPT_CHANGED;
    }

    toasts.push({ title: t('memory.editor.script.toast.failed'), description: cause.message, tone: 'danger' });

    return null;
  }

  if (saved.outcome !== 'exists') {
    await Promise.all([scriptList.refresh({ quiet: true }), scriptDetail.refresh({ quiet: true })]);
  }

  return saved;
}

/** The script as it is now, read again for an editor that found it changed: `gone` when it no longer exists, null (said in a toast) when it could not be read. */
export async function rereadScript(id: number, project: string | null): Promise<ScriptDetail | 'gone' | null> {
  try {
    const script = await ipc.invoke('memory_script_get', { id, project });

    await Promise.all([scriptList.refresh({ quiet: true }), scriptDetail.refresh({ quiet: true })]);

    return script;
  } catch (error) {
    const cause = IpcFailure.from(error);

    if (cause.code === 'notFound') {
      await scriptList.refresh({ quiet: true });

      return 'gone';
    }

    toasts.push({ title: t('memory.editor.script.reload.failed'), description: cause.message, tone: 'danger' });

    return null;
  }
}
