import { entrySections, inScope, matchesPaths, normalizePath, type Section } from '$lib/git/entries';
import { matchingLines, normalizeQuery, type FileHit } from '$lib/git/search';
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
/** Agent edits come in bursts; one re-read follows the last of them by this long. */
const SOON_MS = 400;

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
  /** Which heading the selected file was picked under; a file both staged and edited shows a different diff under each. */
  section = $state<Section | null>(null);
  diffs = $state.raw<Record<string, DiffState>>({});

  /** What the user typed in the search field: it matches paths and the lines of the diffs. */
  search = $state('');
  /** Which matching line of the open file is the current one, and how many there are (the diff view reports it). */
  cursor = $state(0);
  matchTotal = $state(0);

  /** Changed paths under the scope tab and the path filter. */
  scopeEntries: StatusEntry[] = $derived(this.visibleEntries());

  /** Per path, what the search found; empty without a query. A file whose diff has not loaded yet only has its path checked. */
  hits: ReadonlyMap<string, FileHit> = $derived.by(() => {
    const needle = normalizeQuery(this.search);
    const found = new Map<string, FileHit>();

    if (needle === null) {
      return found;
    }

    for (const entry of this.scopeEntries) {
      const state = this.diffs[entry.path];
      const lines = state?.status === 'ready' ? state.diffs.reduce((sum, diff) => sum + matchingLines(diff, needle).length, 0) : 0;
      const path = entry.path.toLowerCase().includes(needle) || (entry.origPath?.toLowerCase().includes(needle) ?? false);

      if (path || lines > 0) {
        found.set(entry.path, { path, lines });
      }
    }

    return found;
  });

  /** Changed paths under the scope tab, the path filter and the search. */
  entries: StatusEntry[] = $derived(normalizeQuery(this.search) === null ? this.scopeEntries : this.scopeEntries.filter((entry) => this.hits.has(entry.path)));

  /** Some file in view has no diff to search yet. */
  searching = $derived(normalizeQuery(this.search) !== null && this.scopeEntries.some((entry) => this.diffs[entry.path]?.status === 'loading' || this.diffs[entry.path] === undefined));

  private project: string | null = null;
  private generation = 0;
  private diffGeneration = 0;
  private soonTimer: ReturnType<typeof setTimeout> | null = null;

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

  /** Re-reads quietly once a burst of changes (an agent editing files) has passed. */
  refreshSoon(): void {
    if (this.soonTimer !== null) {
      clearTimeout(this.soonTimer);
    }

    this.soonTimer = setTimeout(() => {
      this.soonTimer = null;
      void this.refresh({ quiet: true });
    }, SOON_MS);
  }

  /**
   * Re-reads status and branches, then the diffs of the files in view. A `quiet` refresh happens in the background:
   * no spinner, and the diffs on screen stay until their new versions arrive.
   */
  async refresh({ quiet = false } = {}): Promise<void> {
    const project = this.project;
    if (project === null) {
      return;
    }

    this.generation += 1;
    const mine = this.generation;
    if (!quiet) {
      this.refreshing = true;
    }
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
      this.reload(quiet);
    } catch (error) {
      // A background re-read that fails keeps the state on screen; the next one tries again.
      if (mine === this.generation && !(quiet && this.status !== null)) {
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

  /** Searches paths and contents; the diffs of the files in view are loaded first so contents can be read. */
  setSearch(query: string): void {
    this.search = query;
    this.cursor = 0;

    if (normalizeQuery(query) === null) {
      return;
    }

    const missing = this.scopeEntries.filter((entry) => !this.diffs[entry.path]).map((entry) => entry.path);
    if (missing.length > 0) {
      void this.fetchDiffs(missing);
    }

    if (!this.entries.some((entry) => entry.path === this.selected)) {
      this.choose(this.entries[0]?.path ?? null);
    }
  }

  /** Moves to the next or previous match of the open file, wrapping around. */
  stepMatch(direction: 1 | -1): void {
    if (this.matchTotal > 0) {
      this.cursor = (this.cursor + direction + this.matchTotal) % this.matchTotal;
    }
  }

  /** Selects a file under a heading; without one, the first heading that holds it. */
  select(path: string | null, section?: Section): void {
    this.choose(path, section);
    this.cursor = 0;

    if (path !== null && !this.diffs[path]) {
      void this.fetchDiffs([path]);
    }
  }

  private choose(path: string | null, section?: Section): void {
    const entry = path === null ? undefined : this.status?.entries.find((candidate) => candidate.path === path);

    this.selected = path;
    this.section = path === null ? null : (section ?? (entry ? entrySections(entry)[0] : undefined) ?? null);
  }

  /** Selects a changed file even when the scope tab or the path filter hides it, by widening both. */
  reveal(path: string): void {
    const visible = this.entries.some((entry) => entry.path === path);

    if (!visible) {
      this.search = '';
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

  /** The diffs of the files in view are stale: forget them (or, `quiet`, keep showing them), keep a valid selection, fetch again. */
  private reload(quiet = false): void {
    this.diffGeneration += 1;
    const visible = new Set(this.scopeEntries.map((entry) => entry.path));
    this.diffs = quiet ? Object.fromEntries(Object.entries(this.diffs).filter(([path, diff]) => visible.has(path) && diff.status === 'ready')) : {};

    const current = this.selected === null ? undefined : this.entries.find((entry) => entry.path === this.selected);
    if (!current) {
      this.choose(this.entries[0]?.path ?? null);
    } else if (this.section === null || !entrySections(current).includes(this.section)) {
      this.choose(current.path);
    }

    if (this.scopeEntries.length > 0 && this.scopeEntries.length <= BULK_LIMIT) {
      void this.fetchDiffs(this.scopeEntries.map((entry) => entry.path), quiet);
    } else if (this.selected !== null) {
      void this.fetchDiffs([this.selected], quiet);
    }
  }

  /**
   * Fetches the diffs of some visible files. Without a filter a bulk request asks for the whole
   * scope, so git can still pair renames; with one it names the paths (and their old names).
   */
  private async fetchDiffs(paths: string[], quiet = false): Promise<void> {
    const project = this.project;
    if (project === null) {
      return;
    }

    const mine = this.diffGeneration;
    const bulk = paths.length > 1 && !this.filter;
    const entries = this.scopeEntries.filter((entry) => paths.includes(entry.path));
    const requested = bulk ? [] : entries.flatMap((entry) => (entry.origPath ? [entry.path, entry.origPath] : [entry.path]));

    this.putDiffs(mine, quiet ? paths.filter((path) => this.diffs[path]?.status !== 'ready') : paths, { status: 'loading' });

    try {
      const found = await this.transport.invoke('git_diff', { project, request: { scope: this.scope, paths: requested } });
      for (const path of paths) {
        const diffs = found.filter((diff) => diff.path === path);
        const before = this.diffs[path];

        // A quiet re-read that found the same diff keeps the one on screen, so an open comment or a gap the reader opened stays put.
        if (before?.status === 'ready' && JSON.stringify(before.diffs) === JSON.stringify(diffs)) {
          continue;
        }

        this.putDiffs(mine, [path], { status: 'ready', diffs });
      }
    } catch (error) {
      const failed = quiet ? paths.filter((path) => this.diffs[path]?.status !== 'ready') : paths;
      this.putDiffs(mine, failed, { status: 'error', error: IpcFailure.from(error) });
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
    if (this.soonTimer !== null) {
      clearTimeout(this.soonTimer);
      this.soonTimer = null;
    }
    this.generation += 1;
    this.diffGeneration += 1;
    this.status = null;
    this.branches = [];
    this.loadStatus = 'idle';
    this.error = null;
    this.refreshing = false;
    this.scope = 'all';
    this.filter = null;
    this.search = '';
    this.cursor = 0;
    this.matchTotal = 0;
    this.selected = null;
    this.section = null;
    this.diffs = {};
  }
}

export const git = new GitStore(ipc);
