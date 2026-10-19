import type { AgentAction } from './bindings/AgentAction';
import type { AgentEnvelope } from './bindings/AgentEnvelope';
import type { AgentReply } from './bindings/AgentReply';
import type { AgentSessionInfo } from './bindings/AgentSessionInfo';
import type { AgentStartRequest } from './bindings/AgentStartRequest';
import type { AppInfo } from './bindings/AppInfo';
import type { AppPrefs } from './bindings/AppPrefs';
import type { BashOutput } from './bindings/BashOutput';
import type { Branch } from './bindings/Branch';
import type { CliResult } from './bindings/CliResult';
import type { CommandSpec as CatalogCommand } from './bindings/CommandSpec';
import type { ComposedReview } from './bindings/ComposedReview';
import type { ConfigView } from './bindings/ConfigView';
import type { AttachedFile } from './bindings/AttachedFile';
import type { DropEvent } from './bindings/DropEvent';
import type { DashboardAvailability } from './bindings/DashboardAvailability';
import type { DashboardInstallResult } from './bindings/DashboardInstallResult';
import type { DashboardQuery } from './bindings/DashboardQuery';
import type { DashboardResult } from './bindings/DashboardResult';
import type { DiffRequest } from './bindings/DiffRequest';
import type { EcosystemInstalled } from './bindings/EcosystemInstalled';
import type { EcosystemRelease } from './bindings/EcosystemRelease';
import type { EcosystemStatus } from './bindings/EcosystemStatus';
import type { ExtensionAction } from './bindings/ExtensionAction';
import type { ExtensionsView } from './bindings/ExtensionsView';
import type { FileDiff } from './bindings/FileDiff';
import type { GitStatus } from './bindings/GitStatus';
import type { HarnessId } from './bindings/HarnessId';
import type { HarnessSummary } from './bindings/HarnessSummary';
import type { HookDraft } from './bindings/HookDraft';
import type { HookFamilyEntry } from './bindings/HookFamilyEntry';
import type { HookListing } from './bindings/HookListing';
import type { HooksDeleted } from './bindings/HooksDeleted';
import type { HooksEvent } from './bindings/HooksEvent';
import type { HookSeed } from './bindings/HookSeed';
import type { HookView } from './bindings/HookView';
import type { InstallLogLine } from './bindings/InstallLogLine';
import type { McpState } from './bindings/McpState';
import type { MemoryDeleteAll } from './bindings/MemoryDeleteAll';
import type { MemoryNotesTarget } from './bindings/MemoryNotesTarget';
import type { MemoryRemote } from './bindings/MemoryRemote';
import type { MemoryRemoteKind } from './bindings/MemoryRemoteKind';
import type { MemoryRemoteTest } from './bindings/MemoryRemoteTest';
import type { MemoryNotesPage } from './bindings/MemoryNotesPage';
import type { MemoryScriptsPage } from './bindings/MemoryScriptsPage';
import type { MemorySwitchResult } from './bindings/MemorySwitchResult';
import type { RecommendedSetup } from './bindings/RecommendedSetup';
import type { ProjectInfo } from './bindings/ProjectInfo';
import type { ReviewRequest } from './bindings/ReviewRequest';
import type { RoutineDraft } from './bindings/RoutineDraft';
import type { RoutineFilter } from './bindings/RoutineFilter';
import type { RoutineListing } from './bindings/RoutineListing';
import type { RoutineRun } from './bindings/RoutineRun';
import type { RoutineRunsPage } from './bindings/RoutineRunsPage';
import type { RoutineRunsQuery } from './bindings/RoutineRunsQuery';
import type { RoutinesDeleted } from './bindings/RoutinesDeleted';
import type { RoutinesEvent } from './bindings/RoutinesEvent';
import type { RoutineView } from './bindings/RoutineView';
import type { RoutineWhen } from './bindings/RoutineWhen';
import type { SchedulePreview } from './bindings/SchedulePreview';
import type { NoteDetail } from './bindings/NoteDetail';
import type { NoteKind } from './bindings/NoteKind';
import type { NoteSort } from './bindings/NoteSort';
import type { ScriptDetail } from './bindings/ScriptDetail';
import type { ScriptGroup } from './bindings/ScriptGroup';
import type { ScriptReview } from './bindings/ScriptReview';
import type { ScriptScope } from './bindings/ScriptScope';
import type { ScriptSort } from './bindings/ScriptSort';
import type { SortOrder } from './bindings/SortOrder';
import type { ScriptRequestEvent } from './bindings/ScriptRequestEvent';
import type { SessionFolder } from './bindings/SessionFolder';
import type { SessionMessagesPage } from './bindings/SessionMessagesPage';
import type { SessionRef } from './bindings/SessionRef';
import type { SessionSummary } from './bindings/SessionSummary';
import type { SessionTree } from './bindings/SessionTree';
import type { SyncExported } from './bindings/SyncExported';
import type { SyncExportPreview } from './bindings/SyncExportPreview';
import type { SyncImportPlan } from './bindings/SyncImportPlan';
import type { SyncImportResult } from './bindings/SyncImportResult';
import type { SyncInput } from './bindings/SyncInput';
import type { SyncProgress } from './bindings/SyncProgress';
import type { TeamRoutinesRemoved } from './bindings/TeamRoutinesRemoved';
import type { UiResponse } from './bindings/UiResponse';
import type { UpdateInfo } from './bindings/UpdateInfo';
import type { UpdateProgress } from './bindings/UpdateProgress';
import type { EventSink } from './transport';

/** Phantom entry: carries the argument and result types of one command, no runtime data. */
export interface CommandSpec<Args, Result> {
  readonly args: Args;
  readonly result: Result;
}

function cmd<Args, Result>(): CommandSpec<Args, Result> {
  return {} as CommandSpec<Args, Result>;
}

/**
 * The single typed command map. Keys are the snake_case names of the Rust
 * `#[tauri::command]` functions; `scripts/check-ipc-contract.ts` compares them 1:1.
 */
export const commands = {
  app_info: cmd<undefined, AppInfo>(),
  prefs_get: cmd<undefined, AppPrefs>(),
  prefs_set: cmd<{ prefs: AppPrefs }, AppPrefs>(),
  /**
   * The "Project memory" switch: stores `memoryEnabled` and registers or removes the harness-memory MCP entry.
   * `server.installed` false means nothing was registered; offer `setup_install_memory`.
   */
  memory_set_enabled: cmd<{ enabled: boolean }, MemorySwitchResult>(),
  /** The team database in Harness Memory's config.toml: the URL redacted, the token as `hasToken`, and any `problem` with the file. */
  memory_remote_get: cmd<undefined, MemoryRemote>(),
  /**
   * Saves the team database (`enabled`) or removes it (the file is deleted). A null `url`/`token` keeps the saved one, `token: ''` clears it;
   * the token is stored for libSQL only. `invalidInput` for a URL or token Harness Memory would refuse, or a file the app must not overwrite.
   */
  memory_remote_set: cmd<
    {
      enabled: boolean;
      kind: MemoryRemoteKind | null;
      url: string | null;
      token: string | null;
      repositoryNotes: MemoryNotesTarget;
      globalNotes: MemoryNotesTarget;
      /** Saved as `scripts = "local"` only when `local` and the result's `scriptsSupported`, never written otherwise. */
      scripts: MemoryNotesTarget;
      /**
       * Saved as `trust_team_scripts = true` only when true and the result's `trustTeamScriptsSupported`, never written when false.
       * While the server cannot be asked the saved key is kept as it is.
       */
      trustTeamScripts: boolean;
      /** Saved as `hooks = "remote"` only when true and the result's `hooksSupported`; null keeps the saved choice. */
      shareHooks?: boolean | null;
      /** Saved as `trust_team_hooks = true` only when true, hooks are shared and `hooksSupported`; null keeps the saved choice. */
      trustTeamHooks?: boolean | null;
      /** Saved as `routines = "remote"` only when true and the result's `routinesSupported`; null keeps the saved choice. */
      shareRoutines?: boolean | null;
      /** Saved as `trust_team_routines = true` only when true, routines are shared and `routinesSupported`; null keeps the saved choice. */
      trustTeamRoutines?: boolean | null;
    },
    MemoryRemote
  >(),
  /** Runs `harness-memory check` (20 s) with the given URL/token, or the saved ones where null. Never rejects for a failed check: see `ok`. */
  memory_remote_test: cmd<{ kind: MemoryRemoteKind; url: string | null; token: string | null }, MemoryRemoteTest>(),
  /**
   * One page of the project's scripts and the global ones (`harness-memory scripts list --json`, 15 s). Never rejects for a
   * missing or old server: see `availability` (`scripts` is empty, `total` and `groups` are 0 unless `available`).
   * `project` is the project root, null for global only. `offset` defaults to 0, `limit` to 50 (1 to 200), `query`
   * (at most 256 characters, blank for none) matches name or description, `group` keeps one group (`groups` counts every
   * group under the query either way), `sort` (`updated`, `name`, `language`, `scope` with global first, `place`, `runs`,
   * `lastRun` with never-run scripts last either way) defaults to `updated`, `order` to `desc` for `updated` and `asc`
   * otherwise; ties go by id. A server that does not know the sort answers `outdated`.
   * Rejects with `invalidInput` for a bad limit or a long query, `forbidden` for a project that is not open or recent.
   */
  memory_scripts_list: cmd<
    {
      project: string | null;
      offset: number;
      limit?: number;
      query?: string;
      /** The scripts of one scope, and by approval: with a server that combines them. Never sent with `group` (`invalidInput`). */
      scope?: ScriptScope;
      review?: ScriptReview;
      /** The one group of an older server, which cannot combine filters. Never sent with `scope` or `review`. */
      group?: ScriptGroup;
      sort?: ScriptSort;
      order?: SortOrder;
    },
    MemoryScriptsPage
  >(),
  /** One script with its body (`script_get`). `notFound` when it is gone or the server is not installed. */
  memory_script_get: cmd<{ id: number; project: string | null }, ScriptDetail>(),
  /** Deletes one script (`script_delete`). `notFound` when it is gone or the server is not installed. */
  memory_script_delete: cmd<{ id: number; project: string | null }, null>(),
  /**
   * Deletes every script `memory_scripts_list` shows for `project` without filters, every group and page
   * (`harness-memory scripts delete --all --yes`, 60 s): `deleted` counts them, `team` those that were in the team database
   * (null when the server does not say). Rejects with `unsupported` when the server predates it, `notFound` when Harness
   * Memory is not installed, `process` with the server's (redacted) message when it fails, `forbidden` for a project that
   * is not open or recent.
   */
  memory_scripts_delete_all: cmd<{ project?: string | null }, MemoryDeleteAll>(),
  /**
   * Approves the reviewed versions of scripts with `harness-memory trust <id>=<sha256>… [--project <root>]` (20 s and 1 s per script, 120 s at most): each item
   * is a script id and the `sha256` the user saw (64 lowercase hex), at most 500. Rejects with `conflict` when a script
   * changed since it was reviewed (reload it and ask again, the others are still approved), `invalidInput` for no items or
   * a bad id or hash, `process` with the server's (redacted) message when it fails, `notFound` when Harness Memory is not installed.
   */
  memory_scripts_trust: cmd<{ items: { id: number; sha256: string }[]; project: string | null }, null>(),
  /**
   * Sends the run requests of agents (a script this machine has not approved) to `onRequest`, replacing the previous listener.
   * Every request still waiting is sent again at once. `gone` follows when the server got an answer or stopped waiting (45 s).
   */
  memory_script_requests_listen: cmd<{ onRequest: EventSink<ScriptRequestEvent> }, null>(),
  /**
   * Answers one run request (`allow` runs the script once and approves this version). `notFound` when the request is gone
   * or was never reported to `memory_script_requests_listen`, or already answered.
   */
  memory_script_decide: cmd<{ requestId: string; allow: boolean }, null>(),
  /**
   * One page of the project's repository notes and the global ones (`harness-memory notes list --json`, 15 s). Never
   * rejects for a missing or old server: see `availability` (`notes` is empty, `total` 0 and `kinds` empty unless
   * `available`). `project` is the project root, null for global only. `offset` defaults to 0, `limit` to 50 (1 to 200),
   * `query` (at most 256 characters, blank for none) is the server's search, `kind` keeps one kind (`kinds` counts every
   * kind under the query either way), `sort` (`updated`, `title`, `kind` by name, `scope` with global first, `place` with
   * this computer first) defaults to `updated`, `order` to `desc` for `updated` and `asc` otherwise; ties go by id. A
   * server that does not know the sort answers `outdated`.
   * Rejects with `invalidInput` for a bad limit or a long query, `forbidden` for a project that is not open or recent.
   */
  memory_notes_list: cmd<
    {
      project: string | null;
      offset: number;
      limit?: number;
      query?: string;
      kind?: NoteKind;
      /** The notes of one scope; an older server drops it (the page then says `combinedFilters: false`). */
      scope?: ScriptScope;
      sort?: NoteSort;
      order?: SortOrder;
    },
    MemoryNotesPage
  >(),
  /**
   * One note with its body and its links (at most 50) (`harness-memory notes get`, 15 s). `notFound` when it is gone or
   * Harness Memory is not installed, `unsupported` when the server predates the notes view, `process` with the server's
   * (redacted) message when it fails.
   */
  memory_note_get: cmd<{ id: number; project: string | null }, NoteDetail>(),
  /**
   * Deletes one note (`harness-memory notes delete`, 15 s). `notFound` when it is gone or Harness Memory is not installed,
   * `unsupported` when the server predates the notes view, `process` with the server's (redacted) message when it fails.
   */
  memory_note_delete: cmd<{ id: number; project: string | null }, null>(),
  /**
   * Deletes every note `memory_notes_list` shows for `project` without filters, every page (`harness-memory notes delete
   * --all --yes`, 60 s): `deleted` counts them, `team` those that were in the team database (null when the server does not
   * say). Rejects with `unsupported` when the server predates it, `notFound` when Harness Memory is not installed,
   * `process` with the server's (redacted) message when it fails, `forbidden` for a project that is not open or recent.
   */
  memory_notes_delete_all: cmd<{ project?: string | null }, MemoryDeleteAll>(),
  open_external: cmd<{ url: string }, null>(),
  reveal_path: cmd<{ path: string }, null>(),
  harness_list: cmd<undefined, HarnessSummary[]>(),
  harness_detect: cmd<{ harness: HarnessId }, HarnessSummary>(),
  harness_pick_binary: cmd<{ harness: HarnessId }, HarnessSummary | null>(),
  harness_clear_binary: cmd<{ harness: HarnessId }, HarnessSummary>(),
  config_view: cmd<{ harness: HarnessId; project: string | null }, ConfigView>(),
  extensions_view: cmd<{ harness: HarnessId; project: string | null }, ExtensionsView>(),
  commands_catalog: cmd<{ harness: HarnessId }, CatalogCommand[]>(),
  project_pick: cmd<undefined, ProjectInfo | null>(),
  project_open: cmd<{ path: string }, ProjectInfo>(),
  project_close: cmd<undefined, null>(),
  project_recent: cmd<undefined, ProjectInfo[]>(),
  project_forget: cmd<{ path: string }, ProjectInfo[]>(),
  agent_start: cmd<{ request: AgentStartRequest; onEvent: EventSink<AgentEnvelope> }, AgentSessionInfo>(),
  agent_send: cmd<{ key: string; action: AgentAction }, AgentReply>(),
  agent_ui_respond: cmd<{ key: string; response: UiResponse }, null>(),
  agent_bash: cmd<{ key: string; command: string; confirmed: boolean }, BashOutput>(),
  agent_stop: cmd<{ key: string }, null>(),
  agent_move_project: cmd<{ key: string; project: string }, AgentSessionInfo>(),
  agent_list: cmd<undefined, AgentSessionInfo[]>(),
  sessions_list: cmd<{ harness: HarnessId; project: string }, SessionSummary[]>(),
  sessions_folder: cmd<{ harness: HarnessId; project: string }, SessionFolder>(),
  session_tree: cmd<{ session: SessionRef; project: string }, SessionTree>(),
  session_messages: cmd<{ session: SessionRef; project: string; cursor: string | null; limit: number }, SessionMessagesPage>(),
  session_files_touched: cmd<{ session: SessionRef; project: string }, string[]>(),
  session_delete: cmd<{ session: SessionRef; project: string; emptyCopyOnly: boolean }, null>(),
  git_status: cmd<{ project: string }, GitStatus>(),
  git_branches: cmd<{ project: string }, Branch[]>(),
  git_diff: cmd<{ project: string; request: DiffRequest }, FileDiff[]>(),
  git_file_lines: cmd<{ project: string; path: string; staged: boolean }, string[]>(),
  git_discard_all: cmd<{ project: string }, null>(),
  git_discard_files: cmd<{ project: string; paths: string[] }, null>(),
  review_compose: cmd<{ request: ReviewRequest }, ComposedReview>(),
  ecosystem_status: cmd<undefined, EcosystemStatus>(),
  ecosystem_releases: cmd<{ installed: EcosystemInstalled[] }, EcosystemRelease[]>(),
  dashboard_availability: cmd<undefined, DashboardAvailability>(),
  dashboard_query: cmd<{ query: DashboardQuery }, DashboardResult>(),
  extension_action: cmd<{ harness: HarnessId; action: ExtensionAction; dryRun: boolean; project: string | null }, CliResult>(),
  setup_status: cmd<undefined, RecommendedSetup>(),
  setup_install_dashboard: cmd<{ confirmed: boolean; onLog: EventSink<InstallLogLine> }, DashboardInstallResult>(),
  /** Runs the Harness Memory installer; once `ok`, registers the server while project memory is on. */
  setup_install_memory: cmd<{ confirmed: boolean; onLog: EventSink<InstallLogLine> }, DashboardInstallResult>(),
  setup_register_mcp: cmd<{ harness: HarnessId }, McpState>(),
  /** Registers the installed Harness Memory server in one harness (turning Project memory on); `invalidInput` while it is not installed. */
  setup_register_memory: cmd<{ harness: HarnessId }, McpState>(),
  /** Registers the app's own `harness-desktop mcp` server (hooks for agents) in one harness's user MCP file. Needs no installer: the app is the server. */
  setup_register_desktop_mcp: cmd<{ harness: HarnessId }, McpState>(),
  sync_export_preview: cmd<undefined, SyncExportPreview>(),
  sync_export: cmd<{ selected: string[] }, SyncExported>(),
  /** Opens the native save dialog; null when the user cancelled it, else the path written. */
  sync_export_save: cmd<{ selected: string[] }, string | null>(),
  /** A file input opens the native picker; null when the user cancelled it. */
  sync_import_preview: cmd<{ input: SyncInput }, SyncImportPlan | null>(),
  sync_import_apply: cmd<{ planId: string; selected: string[]; confirmed: boolean; onProgress: EventSink<SyncProgress> }, SyncImportResult>(),
  /**
   * The hooks of the Hooks view: every hook file of the global folder and of the project at `project` (null for global only), valid
   * or not, with the status each has. Never rejects for a bad file, see `HookView.problem`. `folderProblem` says why the folder is not read.
   * While hooks are shared with the team (`team` not null) the team hooks of that chat follow the local ones (`origin: team`).
   */
  hooks_list: cmd<{ project: string | null }, HookListing>(),
  /**
   * Creates a hook (`draft.id` null) or edits one (`draft.expectedSha256` is the version the user edited) and approves the written
   * version, so it runs at once. Rejects with `conflict` when the file changed since, `invalidInput` when the draft breaks the format
   * (or the scope already holds 64 hooks), `notFound` for an edit of a hook that is gone.
   */
  hook_save: cmd<{ draft: HookDraft }, HookView>(),
  /** Deletes a hook file and its approval. `conflict` when the file changed since `sha256`, `notFound` when it is gone. */
  hook_delete: cmd<{ id: string; sha256: string }, null>(),
  /**
   * Approves exactly the version with this `sha256` (an agent wrote it, or a team hook while "Trust team hooks" is off; answers the
   * team view then). `conflict` when the file or the team copy changed since.
   */
  hook_approve: cmd<{ id: string; sha256: string }, HookView>(),
  /**
   * Rejects a pending version: a hook with an earlier approved version goes back to it (answers that hook), a hook never approved is
   * deleted (answers null). `conflict` when the file changed since `sha256`.
   */
  hook_reject: cmd<{ id: string; sha256: string }, HookView | null>(),
  /** The Harness family catalog against the hook files: which family hooks are installed, outdated, missing or wait for their member. */
  hooks_family: cmd<undefined, HookFamilyEntry[]>(),
  /** Adds the missing and updates the outdated family hooks named in `seeds` (approved, as the desktop wrote them), then lists the hooks (global ones: ask `hooks_list` again for a project). */
  hooks_family_apply: cmd<{ seeds: HookSeed[] }, HookListing>(),
  /**
   * Deletes every hook of the global scope and of `project` (the global ones only without it), with their approvals, and answers how many
   * went. The Harness family hooks can be added back afterwards.
   */
  hooks_delete_all: cmd<{ project?: string | null }, HooksDeleted>(),
  /** Sends `changed` to `onEvent` whenever the hooks folder, the approvals or the team hooks change, replacing the previous listener. */
  hooks_listen: cmd<{ onEvent: EventSink<HooksEvent> }, null>(),
  /**
   * Shares the local hook `id` at version `sha256` with the team (a project hook under its `git:` repository key), or updates the team
   * copy when the team has it already ("Update team copy"). Answers the team view. `conflict` when the local file or the team copy
   * changed, `forbidden` for a Harness family hook or a repository without an origin remote, `invalidInput` for a pending or invalid
   * hook, `unsupported` while hooks are not shared.
   */
  hooks_share: cmd<{ id: string; sha256: string }, HookView>(),
  /** Removes team hook `id` from the team (`sha256`: the team view's), on every machine after its next refresh. `conflict` when it changed. */
  hooks_team_remove: cmd<{ id: string; sha256: string }, null>(),
  /** Refreshes the team hooks now (network, up to a few seconds), then answers what `hooks_list` answers for `project`. */
  hooks_team_refresh: cmd<{ project?: string | null }, HookListing>(),
  /**
   * Copies team hook `id` (version `sha256`) to the local hooks as an approved, editable hook with the same id, which then runs instead
   * of the team copy. A project hook needs `project` (a folder of that repository). `invalidInput` when a local hook has the id.
   */
  hooks_team_copy: cmd<{ id: string; sha256: string; project?: string | null }, HookView>(),
  /** Turns team hook `id` on or off on this machine only (never written to the team). Answers the team view. */
  hooks_team_enable: cmd<{ id: string; enabled: boolean }, HookView>(),
  /**
   * The routines of the Routines view: every routine file of the global folder and of the project at `project` (null for global only),
   * valid ones first, each with its status (`running` while a run goes on) and `nextRun`. `filter` keeps only the matching routines (null
   * or absent keeps all), `counts` are those of every listed routine. `forbidden` for a project that is not open or recent.
   */
  routines_list: cmd<{ project: string | null; filter?: RoutineFilter | null }, RoutineListing>(),
  /** One routine file as `routines_list` shows it. `notFound` when it is gone. */
  routine_get: cmd<{ id: string }, RoutineView>(),
  /**
   * Creates a routine (`draft.id` null) or edits one (`draft.expectedSha256` is the version the user edited) and approves the written
   * version, so it runs on its schedule at once. `invalidInput` for a draft that breaks the format, a pending file (approve or reject it
   * first) or a 65th routine in a scope, `conflict` when the file changed, `notFound` for an edit of a routine that is gone.
   */
  routine_save: cmd<{ draft: RoutineDraft }, RoutineView>(),
  /** Deletes a routine file, its approval, run history and schedule, after stopping its run. `conflict` when it changed since `sha256`. */
  routine_delete: cmd<{ id: string; sha256: string }, null>(),
  /**
   * Deletes every routine of the global scope and of `project` (the global ones only without it), pending and invalid ones included,
   * with their runs. Team routines stay.
   */
  routines_delete_all: cmd<{ project?: string | null }, RoutinesDeleted>(),
  /** Approves exactly the version with this `sha256` (an agent wrote or changed it). `conflict` when the file changed. */
  routine_approve: cmd<{ id: string; sha256: string }, RoutineView>(),
  /**
   * Rejects a pending version: a routine with an earlier approved version goes back to it (answers that routine), one never approved is
   * deleted (null). `conflict` when the file changed.
   */
  routine_reject: cmd<{ id: string; sha256: string }, RoutineView | null>(),
  /**
   * Sends `changed` when the routines folder or the approvals change, `runStarted` / `runFinished` for every run, and `needsYou` when a
   * run waits for an answer, replacing the previous listener. The OS notification of a run is left out while the window is in front: toast
   * from these events then.
   */
  routines_listen: cmd<{ onEvent: EventSink<RoutinesEvent> }, null>(),
  /**
   * "Run now" for a valid, approved routine (on or off). Answers the run when it started, null when it waits for a free place (at most 2
   * agent and 4 command or script runs at once). `conflict` while it runs or waits, `invalidInput` for a pending or invalid routine.
   */
  routine_run_now: cmd<{ id: string }, RoutineRun | null>(),
  /** Stops a live run, which ends as `stopped`. `notFound` when it is not running. */
  routine_run_stop: cmd<{ runId: string }, null>(),
  /** One page of runs, newest first, of one routine or all, optionally with one status (`limit` 1 to 200). `needsYou` is live. */
  routine_runs: cmd<{ query: RoutineRunsQuery }, RoutineRunsPage>(),
  /** The next runs of a schedule in this machine's time zone while it is edited, or why it is not valid. */
  routine_schedule_preview: cmd<{ when: RoutineWhen }, SchedulePreview>(),
  /**
   * Shows the chat of a live agent run (or of an ended one a window still shows): `onEvent` gets the events so far in `seq` order (the
   * 3 startup events, then the newest, at most 20 000), then the live ones. Answers the chat like `agent_start`. Answer its questions
   * with `agent_ui_respond`, close it with `agent_stop`. When the channel closes the agent keeps running. `notFound` when the run has no
   * chat.
   */
  routine_run_attach: cmd<{ runId: string; onEvent: EventSink<AgentEnvelope> }, AgentSessionInfo>(),
  /** The folder global routines run in and keep their sessions in ("Global routines" in Sessions), null while it does not exist. */
  routines_workspace: cmd<undefined, string | null>(),
  /**
   * Shares the local routine `id` at version `sha256` with the team, or updates the team copy from it ("Update team copy"). Answers
   * the team view. `conflict` when the local file or the team copy changed, `forbidden` for a repository without an `origin` remote,
   * `invalidInput` for a pending or invalid routine, `unsupported` while routines are not shared.
   */
  routines_share: cmd<{ id: string; sha256: string }, RoutineView>(),
  /** Removes team routine `id` from the team (`sha256`: the team view's), on every machine after its next refresh. `conflict` when it changed. */
  routines_team_remove: cmd<{ id: string; sha256: string }, null>(),
  /** Removes every team routine Harness Memory records as this user's ("Remove all my team routines"). Answers how many and which. */
  routines_team_remove_mine: cmd<undefined, TeamRoutinesRemoved>(),
  /** Refreshes the team routines now (network, up to a few seconds), then answers what `routines_list` answers for `project` without a filter. */
  routines_team_refresh: cmd<{ project?: string | null }, RoutineListing>(),
  /**
   * Copies team routine `id` to the local routines as an approved routine with the same id, which then wins. Only a trusted or approved
   * team routine is copied. A project routine needs `project` (an open or recent project of that repository).
   */
  routines_team_copy: cmd<{ id: string; sha256: string; project?: string | null }, RoutineView>(),
  /**
   * Turns team routine `id` on or off on this machine only. A project routine is turned on for the folder `project` (an open or recent
   * project), which must be a copy of the routine's repository. Turning on is not approval. Answers the team view.
   */
  routines_team_enable: cmd<{ id: string; enabled: boolean; project?: string | null }, RoutineView>(),
  update_check: cmd<undefined, UpdateInfo | null>(),
  update_install: cmd<{ onProgress: EventSink<UpdateProgress> }, null>(),
  app_relaunch: cmd<undefined, null>(),
  launch_sound: cmd<{ play: boolean }, null>(),
  drop_listen: cmd<{ onDrop: EventSink<DropEvent> }, null>(),
  attachment_save: cmd<{ name: string; data: string }, AttachedFile>(),
};

export type CommandName = keyof typeof commands;
export type CommandArgs<K extends CommandName> = (typeof commands)[K]['args'];
export type CommandResult<K extends CommandName> = (typeof commands)[K]['result'];
