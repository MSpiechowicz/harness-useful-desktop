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
import type { NoteDetail } from './bindings/NoteDetail';
import type { NoteKind } from './bindings/NoteKind';
import type { NoteSort } from './bindings/NoteSort';
import type { ScriptDetail } from './bindings/ScriptDetail';
import type { ScriptGroup } from './bindings/ScriptGroup';
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
  sync_export_preview: cmd<undefined, SyncExportPreview>(),
  sync_export: cmd<{ selected: string[] }, SyncExported>(),
  /** Opens the native save dialog; null when the user cancelled it, else the path written. */
  sync_export_save: cmd<{ selected: string[] }, string | null>(),
  /** A file input opens the native picker; null when the user cancelled it. */
  sync_import_preview: cmd<{ input: SyncInput }, SyncImportPlan | null>(),
  sync_import_apply: cmd<{ planId: string; selected: string[]; confirmed: boolean; onProgress: EventSink<SyncProgress> }, SyncImportResult>(),
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
