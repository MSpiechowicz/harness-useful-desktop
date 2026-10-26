import type { AppPrefs } from '../bindings/AppPrefs';
import type { BinaryOverride } from '../bindings/BinaryOverride';
import type { MainBranchChoice } from '../bindings/MainBranchChoice';
import type { ProjectInfo } from '../bindings/ProjectInfo';
import { IpcFailure } from '../transport';
import { fixtures } from './fixtures';

/** Folders the mock "native dialog" offers, one per `project_pick`, in order. */
const PICKABLE_FOLDERS: ProjectInfo[] = [
  { path: '/home/demo/Projekty/harness-useful-desktop', name: 'harness-useful-desktop', exists: true, isGitRepo: true },
  { path: '/home/demo/Projekty/website', name: 'website', exists: true, isGitRepo: true },
  { path: '/home/demo/Documents/notes', name: 'notes', exists: true, isGitRepo: false },
];

const MAX_RECENT = 12;

/** How long after start a preview opened with `?drift` changes by itself. */
export const DRIFT_AFTER_MS = 25_000;

/** The preferences of a first launch: what the mock backend starts from and what tests build their samples on. */
export function defaultPrefs(): AppPrefs {
  return {
    version: 1,
    theme: 'system',
    motion: 'full',
    introEnabled: true,
    soundEnabled: false,
    showThinking: true,
    showMessageUsage: true,
    defaultHarness: null,
    activeProject: null,
    rememberRecentProjects: true,
    recentProjects: [],
    updatePolicy: 'startupAsk',
    checkCompanionUpdates: true,
    autoUpdateExtensions: true,
    autoRefreshCatalogs: true,
    openAnyLink: false,
    aiChatNames: false,
    language: 'en',
    onboardingCompleted: false,
    approvalNoticeSeen: false,
    binaryOverrides: [],
    chatDefaults: [],
    memoryEnabled: true,
    vectorSearch: true,
    keepRunningInTray: false,
    memoryNotesAskDefault: false,
    memoryScriptsAskDefault: false,
    backgroundFetch: true,
    mainBranches: [],
  };
}

/**
 * The mock backend's server-managed state. Mirrors the contract: the webview may only change
 * UI-managed preferences, while the active project, recent projects, binary overrides and
 * project memory move through their own commands.
 */
export class MockState {
  prefs: AppPrefs = defaultPrefs();

  overrides: BinaryOverride[] = [];
  memoryEnabled = true;
  vectorSearch = true;
  /** The main branch picked per project: only `git_set_main_branch` changes it. */
  mainBranches: MainBranchChoice[] = [];
  recents: ProjectInfo[] = structuredClone(fixtures.recentProjects);
  active: ProjectInfo | null = null;

  /** Fixture sessions removed by `session_delete`, as `harness:id`. */
  deletedSessions = new Set<string>();

  /** Projects whose fixture changes `git_discard_all` threw away: their tree is clean from then on. */
  discardedProjects = new Set<string>();

  /** Fixture files `git_discard_files` threw away, as `project\0path`. */
  discardedFiles = new Set<string>();

  private pickable: ProjectInfo[] = structuredClone(PICKABLE_FOLDERS);
  private readonly bornAt = Date.now();

  /**
   * Dev preview only: open the app with `?drift` (`bun run dev:mock`, then add it to the address) and, after
   * `DRIFT_AFTER_MS`, the project changes as if somebody worked in a terminal: a changed file is committed (the
   * count in the rail drops) and a new session appears. It shows the views keeping themselves current. Never true
   * in tests or in a build without the mock.
   */
  drifted(): boolean {
    return import.meta.env.MODE === 'mock' && new URLSearchParams(window.location.search).has('drift') && Date.now() - this.bornAt >= DRIFT_AFTER_MS;
  }

  /** Preferences as the real backend returns them: stored UI fields plus the server-managed ones. */
  snapshot(): AppPrefs {
    return {
      ...this.prefs,
      activeProject: this.active?.path ?? null,
      recentProjects: this.recents.map((project) => project.path),
      binaryOverrides: structuredClone(this.overrides),
      memoryEnabled: this.memoryEnabled,
      vectorSearch: this.vectorSearch,
      mainBranches: structuredClone(this.mainBranches),
    };
  }

  /** The "Project memory" switch: the only way to change `memoryEnabled`. */
  setMemoryEnabled(enabled: boolean): AppPrefs {
    this.memoryEnabled = enabled;

    return this.snapshot();
  }

  /** The "Vector search" switch: the only way to change `vectorSearch`. */
  setVectorSearch(enabled: boolean): AppPrefs {
    this.vectorSearch = enabled;

    return this.snapshot();
  }

  /** Stores the UI-managed fields only; turning "remember recents" off clears them. */
  save(next: AppPrefs): AppPrefs {
    this.prefs = { ...next };

    if (!next.rememberRecentProjects) {
      this.recents = [];
    }

    return this.snapshot();
  }

  /** The next folder of the fake dialog, or null once the list is exhausted (a cancelled dialog). */
  pickFolder(): ProjectInfo | null {
    return this.pickable.shift() ?? null;
  }

  activate(project: ProjectInfo): ProjectInfo {
    this.active = project;

    if (this.prefs.rememberRecentProjects) {
      this.recents = [project, ...this.recents.filter((recent) => recent.path !== project.path)].slice(0, MAX_RECENT);
    }

    return structuredClone(project);
  }

  /** A project path is usable when it is the active or a recent project; anything else is refused. */
  requireKnown(path: string | null): void {
    if (path === null) {
      return;
    }

    const known = this.active?.path === path || this.recents.some((project) => project.path === path);
    if (!known) {
      throw IpcFailure.from({ code: 'forbidden', message: 'access refused by a security rule' });
    }
  }
}
