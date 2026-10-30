import { t, tn } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import type { HookDraft } from '$lib/ipc/bindings/HookDraft';
import type { HookFamilyEntry } from '$lib/ipc/bindings/HookFamilyEntry';
import type { HookListing } from '$lib/ipc/bindings/HookListing';
import type { HookSeed } from '$lib/ipc/bindings/HookSeed';
import type { HookView } from '$lib/ipc/bindings/HookView';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { desktopMcp, type DesktopMcpState } from '$lib/stores/desktop-mcp.svelte';
import { DocBusy } from '$lib/stores/doc-busy.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { ViewLoader } from '$lib/stores/view-loader.svelte';

import { EVENT_WHEN, hookTitle, pendingCount, viewKey } from './hook-format';
import { originalOf } from './hook-team';

/** The hooks list has one reading per project, so the loader's key is a constant. */
const KEY = 'hooks';

/**
 * The hooks of every project (and the global ones, and every team hook read), read once for the whole app so the rail can
 * count the ones waiting for the user while another view is open. The backend tells when the folder or the approvals change, and the
 * list is read again then.
 */
export class HooksStore {
  readonly list = new ViewLoader<HookListing, string>((_key, path) => ipc.invoke('hooks_list', { project: path, all: true }));

  /** The Harness family catalog against the hook files; null until it was read once. */
  private updatingFamily = false;
  /** Counts family reads: only the latest one may set `family`, so a poll that began before an update cannot bring back what it replaced. */
  private familyReads = 0;
  family = $state.raw<HookFamilyEntry[] | null>(null);
  familyError = $state<string | null>(null);

  /** Whether any harness has the app's own MCP server, through which agents write hooks: unknown until the machine was read. */
  get desktopMcp(): DesktopMcpState {
    return desktopMcp.state;
  }

  private readonly work = new DocBusy();

  /** The keys (`viewKey`) of hooks a switch or a button is working on, so a second click waits. */
  get busy(): ReadonlySet<string> {
    return this.work.keys;
  }

  get listing(): HookListing | null {
    return this.list.data;
  }

  get views(): HookView[] {
    return this.list.data?.hooks ?? [];
  }

  /** How many hooks wait for the user's approval. */
  get pending(): number {
    return pendingCount(this.list.data);
  }

  /** The family hooks whose file holds an earlier catalog text: the update is waiting. Empty until the catalog was read, so nothing shows while it is unknown. */
  get outdated(): HookFamilyEntry[] {
    return (this.family ?? []).filter((entry) => entry.status === 'outdated' && entry.hookId !== null);
  }

  /** The ids of the hook files that have an update waiting. */
  get outdatedIds(): ReadonlySet<string> {
    return new Set(this.outdated.flatMap((entry) => (entry.hookId === null ? [] : [entry.hookId])));
  }

  /** How many family hooks have an update waiting. */
  get updates(): number {
    return this.outdated.length;
  }

  /** The waiting update of this hook, or null when it has none (a team hook is never a family hook's file). */
  updateOf(view: HookView): HookFamilyEntry | null {
    return view.origin === 'local' ? (this.outdated.find((entry) => entry.hookId === view.id) ?? null) : null;
  }

  /** Reads the hooks of every project (`path` is the one in use, the team hooks are verified for it); a list already read is refreshed quietly. */
  follow(path: string | null): void {
    this.list.revalidate(KEY, path);
    void this.readFamily();
  }

  /**
   * Asks the team database for the team hooks of this project now (it can take seconds), then reads the list again. The list
   * is shown first with the copies already there, so the view never waits for the network. A failure shows as the team notice.
   */
  async refreshTeam(path: string | null): Promise<void> {
    try {
      await ipc.invoke('hooks_team_refresh', { project: path });
    } catch {
      // The listing says why the team hooks are not read, when they are not.
    }

    await this.list.refresh({ quiet: true });
  }

  /** Listens for changes of the hooks folder and reads the list again on each. Returns the cleanup. */
  start(): () => void {
    let listening = true;

    ipc
      .invoke('hooks_listen', {
        onEvent: () => {
          if (listening) {
            void this.refresh();
          }
        },
      })
      .catch(() => {});

    void this.updateStaleFamily();

    return () => {
      listening = false;
    };
  }

  /**
   * With automatic updates on, brings the Harness family hooks that still hold an earlier catalog version up to date and
   * says so. A family hook the user edited is never touched here: the Harness family hooks dialog offers it.
   */
  async updateStaleFamily(): Promise<void> {
    if (!(prefs.value?.autoUpdateExtensions ?? false) || this.updatingFamily) {
      return;
    }

    this.updatingFamily = true;

    try {
      const stale = (await ipc.invoke('hooks_family', undefined)).filter((row) => row.stale).map((row) => row.seed);

      if (stale.length > 0) {
        await ipc.invoke('hooks_family_apply', { seeds: stale });
        await this.refresh();
        toasts.push({ title: tn('hooks.toast.familyUpdated', stale.length), description: t('hooks.toast.familyUpdated.description'), tone: 'success' });
      }
    } catch {
      // Left for the dialog, which still offers the update.
    } finally {
      this.updatingFamily = false;
    }
  }

  /** Reads the list again in the background, and the family catalog with it, so an update waiting for a family hook shows without the dialog. */
  async refresh(): Promise<void> {
    await Promise.all([this.list.refresh({ quiet: true }), this.readFamily()]);
  }

  /** The hook with this row key (`viewKey`). */
  find(key: string | null): HookView | null {
    return key === null ? null : (this.views.find((view) => viewKey(view) === key) ?? null);
  }

  /** Creates or edits a hook (the written version is approved at once). Throws what the backend says, for the editor to show. */
  async save(draft: HookDraft): Promise<HookView> {
    try {
      return await ipc.invoke('hook_save', { draft });
    } finally {
      await this.refresh();
    }
  }

  /** Turns a hook on or off, keeping everything else as it is. A team hook's switch is a choice of this computer only. */
  async toggle(view: HookView, enabled: boolean): Promise<boolean> {
    if (view.origin === 'team') {
      return this.run(viewKey(view), 'hooks.toast.toggleFailed', () => ipc.invoke('hooks_team_enable', { id: view.id, enabled }));
    }

    const hook = view.hook;

    if (!hook) {
      return false;
    }

    const draft: HookDraft = {
      id: hook.id,
      expectedSha256: view.sha256,
      name: hook.name,
      description: hook.description,
      scope: hook.scope,
      project: hook.project,
      event: hook.event,
      matcher: hook.matcher,
      action: hook.action,
      enabled,
    };

    return this.run(viewKey(view), 'hooks.toast.toggleFailed', () => ipc.invoke('hook_save', { draft }));
  }

  /** Approves exactly the version on screen. */
  async approve(view: HookView): Promise<boolean> {
    const done = await this.run(viewKey(view), 'hooks.toast.approveFailed', () => ipc.invoke('hook_approve', { id: view.id, sha256: view.sha256 }));

    if (done && view.hook) {
      toasts.push({ title: t('hooks.toast.approved', { name: hookTitle(view) }), description: t('hooks.toast.runs', { when: t(EVENT_WHEN[view.hook.event]) }), tone: 'success' });
    }

    return done;
  }

  /** Puts an edited hook back as the user approved it, and removes one that was never approved. */
  async reject(view: HookView): Promise<boolean> {
    const restores = view.approval === 'changed';
    const done = await this.run(viewKey(view), 'hooks.toast.rejectFailed', () => ipc.invoke('hook_reject', { id: view.id, sha256: view.sha256 }));

    if (done) {
      toasts.push({ title: t(restores ? 'hooks.toast.rejected.restored' : 'hooks.toast.rejected.removed'), tone: 'success' });
    }

    return done;
  }

  async remove(view: HookView): Promise<boolean> {
    const done = await this.run(viewKey(view), 'hooks.toast.deleteFailed', () => ipc.invoke('hook_delete', { id: view.id, sha256: view.sha256 }));

    if (done) {
      toasts.push({ title: t('hooks.toast.deleted', { name: hookTitle(view) }), tone: 'success' });
    }

    return done;
  }

  /** Shares a local hook with the team, as the version on screen. */
  async share(view: HookView): Promise<boolean> {
    const done = await this.run(viewKey(view), 'hooks.toast.shareFailed', () => ipc.invoke('hooks_share', { id: view.id, sha256: view.sha256 }));

    if (done) {
      toasts.push({ title: t('hooks.toast.shared', { name: hookTitle(view) }), tone: 'success' });
    }

    return done;
  }

  /** Puts the local original's current version on the team in place of the team copy. Does nothing when the original is gone. */
  async updateTeam(team: HookView): Promise<boolean> {
    const original = originalOf(this.views, team);

    if (!original) {
      return false;
    }

    const done = await this.run(viewKey(team), 'hooks.toast.teamUpdateFailed', () => ipc.invoke('hooks_share', { id: original.id, sha256: original.sha256 }));

    if (done) {
      toasts.push({ title: t('hooks.toast.teamUpdated', { name: hookTitle(original) }), tone: 'success' });
    }

    return done;
  }

  /** Takes a team hook off the team, for every teammate. */
  async removeTeam(team: HookView): Promise<boolean> {
    const done = await this.run(viewKey(team), 'hooks.toast.teamRemoveFailed', () => ipc.invoke('hooks_team_remove', { id: team.id, sha256: team.sha256 }));

    if (done) {
      toasts.push({ title: t('hooks.toast.teamRemoved', { name: hookTitle(team) }), tone: 'success' });
    }

    return done;
  }

  /** Copies a team hook to the local hooks, where it is the user's own to edit. Answers the local hook, or null when it failed. */
  async copyTeam(team: HookView, path: string | null): Promise<HookView | null> {
    let copy: HookView | null = null;
    const done = await this.run(viewKey(team), 'hooks.toast.copyFailed', async () => {
      copy = await ipc.invoke('hooks_team_copy', { id: team.id, sha256: team.sha256, project: path });
    });

    if (done) {
      toasts.push({ title: t('hooks.toast.copied', { name: hookTitle(team) }), tone: 'success' });
    }

    return copy;
  }

  /** Deletes every hook of the global scope and of the project, then says how many went. Throws what the backend says, for the dialog to show. */
  async removeAll(projectPath: string | null): Promise<number> {
    try {
      const { deleted } = await ipc.invoke('hooks_delete_all', { project: projectPath });

      toasts.push({ title: tn('hooks.deleteAll.done', deleted), tone: 'success' });

      return deleted;
    } finally {
      await this.refresh();
    }
  }

  /** Reads the Harness family catalog. A failure is kept for the dialog to say. */
  async readFamily(): Promise<void> {
    const turn = ++this.familyReads;

    try {
      const read = await ipc.invoke('hooks_family', undefined);

      if (turn !== this.familyReads) {
        return;
      }

      // The poll reads it every few seconds: an unchanged catalog keeps its object, so nothing redraws.
      if (JSON.stringify(read) !== JSON.stringify(this.family)) {
        this.family = read;
      }

      this.familyError = null;
    } catch (error) {
      if (turn === this.familyReads) {
        this.familyError = IpcFailure.from(error).message;
      }
    }
  }

  /** Adds the missing and updates the outdated family hooks named; returns whether it worked. */
  async applyFamily(seeds: HookSeed[]): Promise<boolean> {
    try {
      await ipc.invoke('hooks_family_apply', { seeds });
    } catch (error) {
      toasts.push({ title: t('hooks.toast.familyFailed'), description: IpcFailure.from(error).message, tone: 'danger' });
      await this.refresh();

      return false;
    }

    await this.refresh();
    toasts.push({ title: tn('hooks.toast.family', seeds.length), tone: 'success' });

    return true;
  }

  /** Runs one change on a hook: marks it busy, then reads the list again whatever happened. */
  private run(key: string, failed: MessageKey, change: () => Promise<unknown>): Promise<boolean> {
    return this.work.run(key, change, { failed, conflict: 'hooks.toast.conflict', conflictHint: 'hooks.toast.conflictHint', refresh: () => this.refresh() });
  }

  /** Forgets everything read, for tests and a fresh start. */
  reset(): void {
    this.list.sync(null, null);
    this.family = null;
    this.familyError = null;
    desktopMcp.reset();
    this.work.reset();
  }
}

export const hooks = new HooksStore();

/** The path hooks are read for: the active project, or none. */
export function activeProjectPath(): string | null {
  return project.active?.path ?? null;
}
