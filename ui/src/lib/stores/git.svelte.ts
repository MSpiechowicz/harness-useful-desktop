import { inScope, matchesPaths, normalizePath } from '$lib/git/entries';
import type { Branch } from '$lib/ipc/bindings/Branch';
import type { DiffScope } from '$lib/ipc/bindings/DiffScope';
import type { FileDiff } from '$lib/ipc/bindings/FileDiff';
import type { GitStatus } from '$lib/ipc/bindings/GitStatus';
import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { nav } from './nav.svelte';

/** Up to this many changed files are diffed in one request; beyond it a diff is fetched when its file is selected. */
const BULK_LIMIT = 200;

export type LoadStatus = 'idle' | 'loading' | 'ready' | 'error';

/** Limits the Changes view to some paths, e.g. the files a chat or a saved session touched. */
export interface PathFilter {
  /** Who touched them, for the filter chip: "this chat", a session title. */
  label: string;
  paths: string[];
}

export type DiffState = { status: 'loading' } | { status: 'ready'; diffs: FileDiff[] } | { status: 'error'; error: IpcFailure };

/**
 * Git facts about the open project: status, branches and, per file, the diff of the scope in view.
 * Every response carries the generation it was asked in, so a slow answer for a project, a scope
 * or a refresh that has since moved on never overwrites newer data.
 */
export class GitStore {
  status = $state.raw<GitStatus | null>(null);
  branches = $state.raw<Branch[]>([]);
  loadStatus = $state<LoadStatus>('idle');
  error = $state.raw<IpcFailure | null>(null);
  refreshing = $state(false);

  scope = $state<DiffScope>('all');
  filter = $state.raw<PathFilter | null>(null);
  selected = $state<string | null>(null);
  diffs = $state.raw<Record<string, DiffState>>({});

  /** Changed paths under the scope tab and the path filter. */
  entries: StatusEntry[] = $derived(this.visibleEntries());

  private project: string | null = null;
  private generation = 0;
  private diffGeneration = 0;

  constructor(private readonly transport: Transport) {}

  /** The project whose git state is loaded (or loading), if any. */
  get activeProject(): string | null {
    return this.project;
  }

  /** Makes sure the project's git state is loaded; unlike `activate` it leaves an already-loaded one alone. */
  ensure(project: string | null): void {
    if (project === this.project && this.loadStatus !== 'idle') {
      return;
    }

    this.activate(project);
  }

  /** Loads (or quietly reloads) the project's git state; switching projects starts clean. */
  activate(project: string | null): void {
    if (project === this.project && this.loadStatus !== 'idle') {
      void this.refresh();

      return;
    }

    this.project = project;
    this.reset();

    if (project !== null) {
      void this.refresh();
    }
  }

  /** Re-reads status and branches, then the diffs of the files in view. */
  async refresh(): Promise<void> {
    const project = this.project;
    if (project === null) {
      return;
    }

    this.generation += 1;
    const mine = this.generation;
    this.refreshing = true;
    if (this.status === null) {
      this.loadStatus = 'loading';
    }

    try {
      const [status, branches] = await Promise.all([this.transport.invoke('git_status', { project }), this.transport.invoke('git_branches', { project })]);
      if (mine !== this.generation) {
        return;
      }

      this.status = status;
      this.branches = branches;
      this.error = null;
      this.loadStatus = 'ready';
      this.reload();
    } catch (error) {
      if (mine === this.generation) {
        this.error = IpcFailure.from(error);
        this.loadStatus = 'error';
      }
    } finally {
      if (mine === this.generation) {
        this.refreshing = false;
      }
    }
  }

  setScope(scope: DiffScope): void {
    this.scope = scope;
    this.reload();
  }

  select(path: string | null): void {
    this.selected = path;

    if (path !== null && !this.diffs[path]) {
      void this.fetchDiffs([path]);
    }
  }

  /** Selects a changed file even when the scope tab or the path filter hides it, by widening both. */
  reveal(path: string): void {
    const visible = this.entries.some((entry) => entry.path === path);

    if (!visible) {
      this.filter = null;
      this.scope = 'all';
      this.reload();
    }

    this.select(path);
  }

  clearFilter(): void {
    this.filter = null;
    this.reload();
  }

  /** Opens the Changes view of a project limited to the given paths (the files a session touched). */
  showPaths(project: string, paths: string[], label: string): void {
    if (project !== this.project) {
      this.activate(project);
    }

    this.filter = { label, paths: [...new Set(paths.map(normalizePath))] };
    this.scope = 'all';
    this.reload();
    nav.go('changes');
  }

  /** The diffs of a path in the scope in view; two when a file is both staged and modified. */
  diffFor(path: string): DiffState | undefined {
    return this.diffs[path];
  }

  /** Retries the diff of one file after a failure. */
  retryDiff(path: string): void {
    this.diffs = Object.fromEntries(Object.entries(this.diffs).filter(([key]) => key !== path));
    void this.fetchDiffs([path]);
  }

  /** The diffs of the files in view are stale: forget them, keep a valid selection, fetch again. */
  private reload(): void {
    this.diffGeneration += 1;
    this.diffs = {};

    const stillThere = this.selected !== null && this.entries.some((entry) => entry.path === this.selected);
    if (!stillThere) {
      this.selected = this.entries[0]?.path ?? null;
    }

    if (this.entries.length > 0 && this.entries.length <= BULK_LIMIT) {
      void this.fetchDiffs(this.entries.map((entry) => entry.path));
    } else if (this.selected !== null) {
      void this.fetchDiffs([this.selected]);
    }
  }

  /**
   * Fetches the diffs of some visible files. Without a filter a bulk request asks for the whole
   * scope, so git can still pair renames; with one it names the paths (and their old names).
   */
  private async fetchDiffs(paths: string[]): Promise<void> {
    const project = this.project;
    if (project === null) {
      return;
    }

    const mine = this.diffGeneration;
    const bulk = paths.length > 1 && !this.filter;
    const entries = this.entries.filter((entry) => paths.includes(entry.path));
    const requested = bulk ? [] : entries.flatMap((entry) => (entry.origPath ? [entry.path, entry.origPath] : [entry.path]));

    this.putDiffs(mine, paths, { status: 'loading' });

    try {
      const found = await this.transport.invoke('git_diff', { project, request: { scope: this.scope, paths: requested } });
      for (const path of paths) {
        this.putDiffs(mine, [path], { status: 'ready', diffs: found.filter((diff) => diff.path === path) });
      }
    } catch (error) {
      this.putDiffs(mine, paths, { status: 'error', error: IpcFailure.from(error) });
    }
  }

  private putDiffs(generation: number, paths: string[], state: DiffState): void {
    if (generation !== this.diffGeneration) {
      return;
    }

    const next = { ...this.diffs };
    for (const path of paths) {
      next[path] = state;
    }

    this.diffs = next;
  }

  private visibleEntries(): StatusEntry[] {
    const all = this.status?.entries ?? [];
    const wanted = this.filter ? new Set(this.filter.paths) : null;

    return all.filter((entry) => inScope(entry, this.scope) && (wanted === null || matchesPaths(entry, wanted)));
  }

  private reset(): void {
    this.generation += 1;
    this.diffGeneration += 1;
    this.status = null;
    this.branches = [];
    this.loadStatus = 'idle';
    this.error = null;
    this.refreshing = false;
    this.scope = 'all';
    this.filter = null;
    this.selected = null;
    this.diffs = {};
  }
}

export const git = new GitStore(ipc);
