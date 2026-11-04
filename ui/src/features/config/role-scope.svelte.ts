import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { ipc } from '$lib/ipc/client';
import { project } from '$lib/stores/project.svelte';

import type { ModelScope } from '../routines/harness-models.svelte';

/**
 * Where the harness is asked for its models and levels while a role's picker is open. What it offers can depend on the
 * project's settings, so it is asked in the open project. With no project open it is asked in the routines workspace,
 * the folder agents run in when no project is theirs.
 */
export class RoleScope {
  private workspace = $state<string | null>(null);
  private finding: Promise<void> | null = null;
  private lookedUp = $state(false);

  constructor(private readonly harness: () => HarnessId) {}

  /** The folder and harness to ask in; null until a folder is known. */
  get current(): ModelScope | null {
    const folder = project.active?.path ?? this.workspace;

    return folder ? { harness: this.harness(), project: folder } : null;
  }

  /** No folder to ask in: no project is open and the routines workspace could not be looked up. */
  get unavailable(): boolean {
    return this.current === null && this.lookedUp;
  }

  /** Looks up the routines workspace when no project is open and it is not known yet. */
  find(): Promise<void> {
    if (project.active || this.workspace !== null) {
      return Promise.resolve();
    }

    this.finding ??= ipc
      .invoke('routines_workspace', undefined)
      .then((folder) => {
        this.workspace = folder;
      })
      // No folder to ask in: the model is typed.
      .catch(() => undefined)
      .finally(() => {
        this.finding = null;
        this.lookedUp = true;
      });

    return this.finding;
  }

  /** The scope once the folder is known, looked up first when needed; null when there is none. */
  async settled(): Promise<ModelScope | null> {
    await this.find();

    return this.current;
  }
}
