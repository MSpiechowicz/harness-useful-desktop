import { rowKey, signatureOf } from '$lib/git/branch-keys';
import { noteFor, FAILED_TITLES, type SyncNote } from '$lib/git/sync-notes';
import { t, tn, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { BranchPage } from '$lib/ipc/bindings/BranchPage';
import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
import type { GitStatus } from '$lib/ipc/bindings/GitStatus';
import type { SwitchRequest } from '$lib/ipc/bindings/SwitchRequest';
import type { SyncAction } from '$lib/ipc/bindings/SyncAction';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { git, type GitStore } from './git.svelte';
import { BRANCH_FETCH_GAP_MS } from './live';
import { prefs } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

/** How many branches one request brings; the list asks for the next page when it is scrolled to the end. */
export const BRANCH_PAGE_SIZE = 50;
/** The wait after the last keystroke before the branches are searched. */
const SEARCH_DEBOUNCE_MS = 150;

/** What the store is busy with: one of the sync actions, a switch, or saving the main branch. */
export type BranchBusy = SyncAction | 'switch' | 'main';

/**
 * Where the open project's branch stands against its remote and its main branch, and the actions that move it.
 * Reads, fetches and actions each carry the project and a sequence number they started under, so a slow answer for a
 * project that is no longer open, or older than one already shown, never overwrites newer data. One action runs at a time.
 */
export class BranchSyncStore {
  state = $state.raw<BranchSync | null>(null);
  /** The last background fetch could not reach the remote. Quiet by design: the popover header says so, no toast. */
  fetchFailed = $state(false);
  /** The repository could not be read and nothing is known about it yet: the popover says so instead of spinning. */
  readFailed = $state(false);
  /** The action that is running; the popover disables the rest and shows a spinner on this one. */
  busy = $state<BranchBusy | null>(null);
  /** What the last action did, shown under the rows until the next action or the popover closing. */
  note = $state.raw<SyncNote | null>(null);

  /** The branches matching the last search, every page loaded so far, and whether a search is on its way or failed. */
  page = $state.raw<BranchPage | null>(null);
  searching = $state(false);
  searchFailed = $state(false);
  /** The next page of the last search is on its way. */
  loadingMore = $state(false);

  private project: string | null = null;
  private readSeq = 0;
  private actionSeq = 0;
  private searchSeq = 0;
  private searchTimer: ReturnType<typeof setTimeout> | null = null;
  private lastQuery = '';
  /** The text the rows shown were searched for: the next page continues that search, even after a newer one failed. */
  private pageQuery = '';
  /** How many matches the backend has handed over, the offset of the next page. Can differ from the rows shown, which skip repeats. */
  private fetched = $state(0);
  private signature: string | null = null;
  private lastFetchAt = 0;
  private fetching: { project: string; done: Promise<void> } | null = null;

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
    private readonly repository: Pick<GitStore, 'refresh'>,
    /** Whether the user allows fetching the open project without a click. */
    private readonly fetchAllowed: () => boolean,
  ) {}

  /**
   * Follows the open project: starts over for a new one, and reads again when the git status shows the branch moved,
   * so the popover never contradicts the Changes count or the chip. `status` is the open project's, or null while it loads.
   */
  follow(project: string | null, status: GitStatus | null): void {
    if (project !== this.project) {
      this.start(project);

      return;
    }

    if (project === null || status === null) {
      return;
    }

    const current = signatureOf(status);
    const seen = this.signature;
    this.signature = current;

    // The first status only records what the read that started with the project already covers.
    if (seen !== null && seen !== current) {
      void this.read();
    }
  }

  /** Reads the state again from the repository, without the network. */
  async read(): Promise<void> {
    const project = this.project;
    if (project === null) {
      return;
    }

    this.readSeq += 1;
    const mine = this.readSeq;
    const actions = this.actionSeq;

    try {
      const fresh = await this.transport.invoke('git_sync_state', { project });
      if (project === this.project && mine === this.readSeq && actions === this.actionSeq) {
        this.adopt(fresh);
      }
    } catch {
      // A repository that cannot be read keeps what is shown, and the next change of the status tries again.
      // With nothing shown yet, the popover says it could not read it.
      if (project === this.project && mine === this.readSeq && this.state === null) {
        this.readFailed = true;
      }
    }
  }

  /**
   * Fetches the remote without prompts, when the user allows it, a remote exists and the last fetch is a minute old.
   * A fetch already running is joined. A failure only sets `fetchFailed`.
   */
  async fetchRemote(): Promise<void> {
    const project = this.project;
    if (project === null || !this.fetchAllowed()) {
      return;
    }

    if (this.fetching?.project === project) {
      return this.fetching.done;
    }

    if (Date.now() - this.lastFetchAt < BRANCH_FETCH_GAP_MS) {
      return;
    }

    this.lastFetchAt = Date.now();
    const done: Promise<void> = this.runFetch(project).finally(() => {
      if (this.fetching?.done === done) {
        this.fetching = null;
      }
    });
    this.fetching = { project, done };

    return done;
  }

  private async runFetch(project: string): Promise<void> {
    if (this.state === null) {
      await this.read();
    }

    if (project !== this.project || this.state?.isRepo !== true || this.state.remote === null) {
      return;
    }

    const actions = this.actionSeq;

    try {
      const fresh = await this.transport.invoke('git_fetch', {
        project,
        background: true,
      });
      if (project !== this.project) {
        return;
      }

      this.fetchFailed = false;
      this.readSeq += 1;

      // An action that ended while the fetch ran has newer facts than the fetch.
      if (actions === this.actionSeq) {
        this.adopt(fresh);
      }

      void this.repository.refresh({ quiet: true, statusOnly: true });

      // The list shown or on its way may miss branches the fetch brought: search again, quietly (a failure leaves the list unmarked).
      // A search waiting for the typing to pause already starts over by itself, and a list scrolled past its first page keeps its place.
      if ((this.page !== null || this.searching) && this.searchTimer === null && this.fetched <= BRANCH_PAGE_SIZE) {
        void this.runSearch(this.lastQuery, true);
      }
    } catch {
      if (project === this.project) {
        this.fetchFailed = true;
      }
    }
  }

  /** Runs one sync action. Ignored while another runs. */
  async run(action: SyncAction): Promise<void> {
    const project = this.project;
    if (project === null || this.busy !== null) {
      return;
    }

    const before = this.state;
    this.begin(action);

    try {
      const result = await this.transport.invoke('git_sync_action', {
        project,
        action,
      });
      if (project !== this.project) {
        return;
      }

      this.finish(result.state);
      this.note = noteFor(action, result.outcome, before, result.state);
    } catch (error) {
      if (project === this.project) {
        this.fail(FAILED_TITLES[action], error);
        await this.read();
      }
    } finally {
      await this.settle(project);
    }
  }

  /** Switches to a branch, or creates one and switches to it. Git refusing because of uncommitted files offers "Stash and switch". */
  async switchTo(request: SwitchRequest): Promise<void> {
    const project = this.project;
    if (project === null || this.busy !== null) {
      return;
    }

    // What a new branch starts from, for the line that reports it.
    const base = request.create === 'main' ? this.state?.main?.name : this.state?.branch;
    const carried = (this.state?.trackedChanges ?? 0) + (this.state?.untracked ?? 0);
    this.begin('switch');

    try {
      const result = await this.transport.invoke('git_switch', {
        project,
        request,
      });
      if (project !== this.project) {
        return;
      }

      this.finish(result.state);

      if (result.outcome === 'blockedByChanges') {
        this.notifications.push({
          title: t('branch.blocked.title', { branch: request.branch }),
          description: t('branch.blocked.description'),
          tone: 'warning',
          project,
          action: {
            label: t('branch.blocked.stash'),
            run: () => void this.switchTo({ ...request, stash: true }),
          },
        });

        return;
      }

      this.note = request.create
        ? this.createdNote(request.branch, base ?? '', carried)
        : {
            tone: 'success',
            text: t('branch.note.switched', { branch: request.branch }),
            names: [request.branch],
          };
      await this.runSearch(this.lastQuery);
    } catch (error) {
      if (project === this.project) {
        this.fail('branch.failed.switch', error);
        await this.read();
      }
    } finally {
      await this.settle(project);
    }
  }

  /** The note after a new branch: where it started, and the uncommitted files that came along. */
  private createdNote(branch: string, base: string, carried: number): SyncNote {
    const detail = carried > 0 ? tn('branch.note.created.carried', carried) : t('branch.note.created.detail');

    return {
      tone: 'success',
      text: t('branch.note.created', { branch, base }),
      detail,
      names: [branch, base],
    };
  }

  /** Makes `branch` the main branch of the project, or `null` goes back to the detected one. */
  async pickMain(branch: string | null): Promise<void> {
    const project = this.project;
    if (project === null || this.busy !== null) {
      return;
    }

    this.begin('main');

    try {
      const state = await this.transport.invoke('git_set_main_branch', {
        project,
        branch,
      });
      if (project === this.project) {
        this.finish(state);
      }
    } catch (error) {
      if (project === this.project) {
        this.fail('branch.failed.main', error);
      }
    } finally {
      await this.settle(project);
    }
  }

  /** Searches branch names for `query` after the typing pauses; `now` skips the wait (opening the list, a cleared field). */
  search(query: string, now = false): void {
    this.lastQuery = query;
    this.forgetSearchTimer();

    if (now) {
      void this.runSearch(query);

      return;
    }

    this.searching = true;
    this.searchTimer = setTimeout(() => {
      this.searchTimer = null;
      void this.runSearch(query);
    }, SEARCH_DEBOUNCE_MS);
  }

  /** Forgets the line an action left, e.g. when the popover closes or opens. */
  clearNote(): void {
    this.note = null;
  }

  /** Matches of the last search that are not loaded yet. */
  get hasMore(): boolean {
    return this.page !== null && this.fetched < this.page.total;
  }

  /**
   * Appends the next page of the last search. Ignored while a search or a page is on its way, once everything is loaded
   * and before any search ran. A failure sets `searchFailed`, and asking again tries again.
   */
  async loadMore(): Promise<void> {
    const project = this.project;
    const shown = this.page;
    if (project === null || shown === null || this.searching || this.loadingMore || !this.hasMore) {
      return;
    }

    const mine = this.searchSeq;
    this.loadingMore = true;
    // A newer search that failed stays reported: the rows still answer the older text.
    this.searchFailed &&= this.pageQuery !== this.lastQuery;

    try {
      const next = await this.transport.invoke('git_branch_search', {
        project,
        query: this.pageQuery,
        offset: this.fetched,
        limit: BRANCH_PAGE_SIZE,
      });
      if (project !== this.project || mine !== this.searchSeq) {
        return;
      }

      // A branch made or fetched meanwhile shifts the pages: a row already listed is not listed twice.
      const known = new Set(shown.branches.map(rowKey));
      this.page = { branches: [...shown.branches, ...next.branches.filter((row) => !known.has(rowKey(row)))], total: next.total };
      // A page with nothing in it ends the list, so a shrunken result is not asked for again and again.
      this.fetched = next.branches.length === 0 ? next.total : this.fetched + next.branches.length;
    } catch {
      if (project === this.project && mine === this.searchSeq) {
        this.searchFailed = true;
      }
    } finally {
      if (mine === this.searchSeq) {
        this.loadingMore = false;
      }
    }
  }

  private async runSearch(query: string, quiet = false): Promise<void> {
    const project = this.project;
    if (project === null) {
      return;
    }

    this.searchSeq += 1;
    const mine = this.searchSeq;
    this.searching = true;
    // A page of the old search still on its way is dropped by the new sequence number, so it cannot clear this.
    this.loadingMore = false;

    try {
      const page = await this.transport.invoke('git_branch_search', {
        project,
        query,
        offset: 0,
        limit: BRANCH_PAGE_SIZE,
      });
      if (project === this.project && mine === this.searchSeq) {
        this.page = page;
        this.pageQuery = query;
        this.fetched = page.branches.length;
        this.searchFailed = false;
      }
    } catch {
      if (project === this.project && mine === this.searchSeq) {
        this.searchFailed ||= !quiet || this.page === null || this.pageQuery !== query;
      }
    } finally {
      if (mine === this.searchSeq) {
        this.searching = false;
      }
    }
  }

  private start(project: string | null): void {
    this.project = project;
    this.signature = null;
    this.readSeq += 1;
    this.actionSeq += 1;
    this.searchSeq += 1;
    this.forgetSearchTimer();
    this.lastQuery = '';
    this.pageQuery = '';
    this.lastFetchAt = 0;
    this.fetching = null;
    this.state = null;
    this.fetchFailed = false;
    this.readFailed = false;
    this.busy = null;
    this.note = null;
    this.page = null;
    this.searching = false;
    this.searchFailed = false;
    this.loadingMore = false;
    this.fetched = 0;

    if (project !== null) {
      void this.read();
    }
  }

  private begin(busy: BranchBusy): void {
    this.busy = busy;
    this.note = null;
  }

  /** Adopts the state an action returned: it is newer than anything read or fetched before it. */
  private finish(state: BranchSync): void {
    this.actionSeq += 1;
    this.readSeq += 1;
    this.adopt(state);
  }

  /** After any action: the status, the Changes count and the chip follow, and the busy mark goes. */
  private async settle(project: string): Promise<void> {
    if (project !== this.project) {
      return;
    }

    this.busy = null;
    await this.repository.refresh();
  }

  private fail(title: MessageKey, error: unknown): void {
    const failure = IpcFailure.from(error);
    const firstLine = failure.message.split('\n').find((line) => line.trim() !== '') ?? '';

    this.notifications.push({
      title: t(title),
      description: firstLine.trim(),
      tone: 'danger',
      project: this.project ?? undefined,
    });
  }

  /** Shows `fresh` unless it says what is already shown, so nothing is drawn again for it. */
  private adopt(fresh: BranchSync): void {
    this.readFailed = false;
    if (JSON.stringify(fresh) !== JSON.stringify(this.state)) {
      this.state = fresh;
    }
  }

  private forgetSearchTimer(): void {
    if (this.searchTimer !== null) {
      clearTimeout(this.searchTimer);
      this.searchTimer = null;
    }
  }
}

export const branchSync = new BranchSyncStore(ipc, toasts, git, () => prefs.value?.backgroundFetch === true);
