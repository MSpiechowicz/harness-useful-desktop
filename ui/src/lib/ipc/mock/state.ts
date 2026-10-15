import type { AppPrefs } from '../bindings/AppPrefs';
import type { BinaryOverride } from '../bindings/BinaryOverride';
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
    openAnyLink: false,
    aiChatNames: false,
    language: 'en',
    onboardingCompleted: false,
    approvalNoticeSeen: false,
    binaryOverrides: [],
    chatDefaults: [],
  };
}

/**
 * The mock backend's server-managed state. Mirrors the contract: the webview may only change
 * UI-managed preferences, while the active project, recent projects and binary overrides
 * move through their own commands.
 */
export class MockState {
  prefs: AppPrefs = defaultPrefs();

  overrides: BinaryOverride[] = [];
  recents: ProjectInfo[] = structuredClone(fixtures.recentProjects);
  active: ProjectInfo | null = null;

  /** Fixture sessions removed by `session_delete`, as `harness:id`. */
  deletedSessions = new Set<string>();

  /** Projects whose fixture changes `git_discard_all` threw away: their tree is clean from then on. */
  discardedProjects = new Set<string>();

  /** Fixture files `git_discard_files` threw away, as `project\0path`. */
  discardedFiles = new Set<string>();

  private pickable: ProjectInfo[] = structuredClone(PICKABLE_FOLDERS);

  /** Preferences as the real backend returns them: stored UI fields plus the server-managed ones. */
  snapshot(): AppPrefs {
    return {
      ...this.prefs,
      activeProject: this.active?.path ?? null,
      recentProjects: this.recents.map((project) => project.path),
      binaryOverrides: structuredClone(this.overrides),
    };
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
