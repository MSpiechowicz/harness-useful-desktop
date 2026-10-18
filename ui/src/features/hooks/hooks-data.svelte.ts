import { untrack } from 'svelte';

import { t, tn } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import type { HookDraft } from '$lib/ipc/bindings/HookDraft';
import type { HookFamilyEntry } from '$lib/ipc/bindings/HookFamilyEntry';
import type { HookListing } from '$lib/ipc/bindings/HookListing';
import type { HookSeed } from '$lib/ipc/bindings/HookSeed';
import type { HookView } from '$lib/ipc/bindings/HookView';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { ViewLoader } from '$lib/stores/view-loader.svelte';

import { EVENT_WHEN, hookTitle, pendingCount, viewKey } from './hook-format';
import { originalOf } from './hook-team';

/** The hooks list has one reading per project, so the loader's key is a constant. */
const KEY = 'hooks';

/** What a read-then-wait failure says when it is a conflict: someone (an agent) rewrote the file since the user looked. */
function isConflict(error: unknown): boolean {
  return IpcFailure.from(error).code === 'conflict';
}

/**
 * The hooks of the active project (and the global ones), read once for the whole app so the rail can count the ones
 * waiting for the user while another view is open. The backend tells when the folder or the approvals change, and the
 * list is read again then.
 */
export class HooksStore {
  readonly list = new ViewLoader<HookListing, string>((_key, path) => ipc.invoke('hooks_list', { project: path }));

  /** The Harness family catalog against the hook files; null until the dialog asked. */
  family = $state.raw<HookFamilyEntry[] | null>(null);
  familyError = $state<string | null>(null);

  /** Whether any harness has the app's own MCP server, through which agents write hooks: unknown until the machine was read. */
  desktopMcp = $state<'unknown' | 'registered' | 'missing'>('unknown');

  /** The keys (`viewKey`) of hooks a switch or a button is working on, so a second click waits. */
  busy = $state<ReadonlySet<string>>(new Set());

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

  /** Reads the hooks of this project (or the global ones, with none); a list already read is refreshed quietly. */
  follow(path: string | null): void {
    this.list.revalidate(KEY, path);
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

    return () => {
      listening = false;
    };
  }

  /** Reads the list again in the background, and the family catalog when the dialog has shown it. */
  async refresh(): Promise<void> {
    await Promise.all([this.list.refresh({ quiet: true }), untrack(() => this.family) ? this.readFamily() : Promise.resolve()]);
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
      toasts.push({ title: t('hooks.toast.approved', { name: hookTitle(view), when: t(EVENT_WHEN[view.hook.event]) }), tone: 'success' });
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
    try {
      this.family = await ipc.invoke('hooks_family', undefined);
      this.familyError = null;
    } catch (error) {
      this.familyError = IpcFailure.from(error).message;
    }
  }

  /** Reads whether a harness has the app's MCP server registered. A machine that cannot be read says nothing, rather than nagging. */
  async readDesktopMcp(): Promise<void> {
    try {
      const setup = await ipc.invoke('setup_status', undefined);
      const installed = setup.harnesses.filter((entry) => entry.installed);

      this.desktopMcp = installed.length === 0 || installed.some((entry) => entry.desktopMcp.type === 'configured') ? 'registered' : 'missing';
    } catch {
      this.desktopMcp = 'unknown';
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

  /** Runs one change on a hook: marks it busy, then reads the list again whatever happened. A failure is a toast, a conflict one that says the file changed. */
  private async run(key: string, failed: MessageKey, change: () => Promise<unknown>): Promise<boolean> {
    this.busy = new Set([...this.busy, key]);

    try {
      await change();

      return true;
    } catch (error) {
      if (isConflict(error)) {
        toasts.push({ title: t('hooks.toast.conflict'), description: t('hooks.toast.conflictHint'), tone: 'warning' });
      } else {
        toasts.push({ title: t(failed), description: IpcFailure.from(error).message, tone: 'danger' });
      }

      return false;
    } finally {
      await this.refresh();
      this.busy = new Set([...this.busy].filter((other) => other !== key));
    }
  }

  /** Forgets everything read, for tests and a fresh start. */
  reset(): void {
    this.list.sync(null, null);
    this.family = null;
    this.familyError = null;
    this.desktopMcp = 'unknown';
    this.busy = new Set();
  }
}

export const hooks = new HooksStore();

/** The path hooks are read for: the active project, or none. */
export function activeProjectPath(): string | null {
  return project.active?.path ?? null;
}
