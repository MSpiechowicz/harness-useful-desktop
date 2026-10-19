import type { MemoryDeleteAll } from './bindings/MemoryDeleteAll';
import type { MemoryRemote } from './bindings/MemoryRemote';
import type { MemoryRemoteKind } from './bindings/MemoryRemoteKind';
import type { MemoryRemoteTest } from './bindings/MemoryRemoteTest';
import type { MemoryNotesPage } from './bindings/MemoryNotesPage';
import type { MemoryScriptsPage } from './bindings/MemoryScriptsPage';
import type { MemorySwitchResult } from './bindings/MemorySwitchResult';
import type { NoteDetail } from './bindings/NoteDetail';
import type { NoteDraft } from './bindings/NoteDraft';
import type { NoteKind } from './bindings/NoteKind';
import type { NoteSaved } from './bindings/NoteSaved';
import type { NoteSort } from './bindings/NoteSort';
import type { NotesImportFormat } from './bindings/NotesImportFormat';
import type { NotesImportPreview } from './bindings/NotesImportPreview';
import type { NotesImportProgress } from './bindings/NotesImportProgress';
import type { NotesImportResult } from './bindings/NotesImportResult';
import type { ScriptDetail } from './bindings/ScriptDetail';
import type { ScriptDraft } from './bindings/ScriptDraft';
import type { ScriptGroup } from './bindings/ScriptGroup';
import type { ScriptReview } from './bindings/ScriptReview';
import type { ScriptSaved } from './bindings/ScriptSaved';
import type { ScriptScope } from './bindings/ScriptScope';
import type { ScriptSort } from './bindings/ScriptSort';
import type { SortOrder } from './bindings/SortOrder';
import type { ScriptRequestEvent } from './bindings/ScriptRequestEvent';
import type { MemoryCopyKind } from './bindings/MemoryCopyKind';
import type { MemoryNotesRouting } from './bindings/MemoryNotesRouting';
import type { NotesCopyPreview } from './bindings/NotesCopyPreview';
import type { NotesCopyResult } from './bindings/NotesCopyResult';
import type { LocalNoteIds } from './bindings/LocalNoteIds';
import type { NotesCopyFilter } from './bindings/NotesCopyFilter';
import type { NotesCopyRows } from './bindings/NotesCopyRows';
import type { EventSink } from './transport';
import { cmd } from './command-spec';

/** The `memory_*` commands, spread into `commands`. */
export const memoryCommands = {
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
      /** `remoteLocal` ("Remote + this computer") is refused with `unsupported` by a server without `notes-remote-local`. */
      repositoryNotes: MemoryNotesRouting;
      globalNotes: MemoryNotesRouting;
      /**
       * Saved as `scripts = "local"` or `"remote+local"` only when not `remote` and the result's `scriptsSupported`, never written otherwise.
       * `remoteLocal` is refused with `unsupported` by a server without `scripts-remote-local`.
       */
      scripts: MemoryNotesRouting;
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
      /** Only the remote scripts hidden on this computer (the page's `total` is then their count). */
      hidden?: boolean | null;
    },
    MemoryScriptsPage
  >(),
  /** One script with its body (`script_get`). `hidden` reads a remote script hidden on this computer. `notFound` when it is gone or the server is not installed. */
  memory_script_get: cmd<{ id: number; project: string | null; hidden?: boolean }, ScriptDetail>(),
  /** Deletes one script (`script_delete`). `notFound` when it is gone or the server is not installed. */
  memory_script_delete: cmd<{ id: number; project: string | null }, null>(),
  /**
   * Hides remote script `id` on this computer (`hidden: true`, `harness-memory scripts hide`) or shows it again. The remote database never
   * changes. `notFound` when the script is gone, `unsupported` when the server predates hiding scripts, `process` when it refuses.
   */
  memory_script_hide: cmd<{ id: number; project: string | null; hidden: boolean }, null>(),
  /**
   * Deletes scripts from the local file while scripts are routed to the remote database, where they are invisible (`scripts discard-local`):
   * `ids` are local-file ids from `memory_notes_copy_rows` (kind `scripts`), or `all`, narrowed by `scope`. Answers how many were deleted.
   */
  memory_scripts_discard_local: cmd<{ ids: LocalNoteIds; scope?: ScriptScope | null }, number>(),
  /**
   * Creates a script (`script_save` over `harness-memory mcp`, 15 s), or with `replace` replaces the one of that name in that scope
   * (an edit, or a confirmed replace). An edit also sends `base`, the `sha256` of the version the editor opened: when that script was
   * deleted or changed since, nothing is saved and it rejects with `conflict`. Saved on this machine, so trusted unless `script.approval`
   * says otherwise: code waiting for approval stays so after an edit that left the code as it opened (line endings and trailing
   * whitespace aside) or a replace that keeps the code of the untrusted script it replaces. Without `replace`, a
   * script of that name in that scope is `exists` with its `id` and no `script`, and nothing is saved. Rejects with `invalidInput`
   * for a draft over a limit or a repository script without a project, `notFound` when Harness Memory is not installed,
   * `unsupported` when it predates saving from the app, `process` with the server's (redacted) message when it refuses,
   * `forbidden` for a project that is not open or recent.
   */
  memory_script_save: cmd<{ project: string | null; draft: ScriptDraft; replace: boolean; base?: string }, ScriptSaved>(),
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
      /** Only the team notes hidden on this computer (the page's `total` is then their count). */
      hidden?: boolean;
    },
    MemoryNotesPage
  >(),
  /**
   * One note with its body and its links (at most 50) (`harness-memory notes get`, 15 s). `notFound` when it is gone or
   * Harness Memory is not installed, `unsupported` when the server predates the notes view, `process` with the server's
   * (redacted) message when it fails.
   */
  memory_note_get: cmd<{ id: number; project: string | null; hidden?: boolean }, NoteDetail>(),
  /**
   * Deletes one note (`harness-memory notes delete`, 15 s). `notFound` when it is gone or Harness Memory is not installed,
   * `unsupported` when the server predates the notes view, `process` with the server's (redacted) message when it fails.
   */
  memory_note_delete: cmd<{ id: number; project: string | null }, null>(),
  /**
   * Hides team note `id` on this computer (`hidden: true`, `harness-memory notes hide`) or shows it again. The team database never
   * changes. `notFound` when the note is gone, `unsupported` when the server predates hiding, `process` when it refuses.
   */
  memory_note_hide: cmd<{ id: number; project: string | null; hidden: boolean }, null>(),
  /**
   * Deletes notes from the local file of a scope routed to the team database, where they are invisible (`notes discard-local`):
   * `ids` are local-file ids from `memory_notes_copy_rows`, or `all`, narrowed by `scope`. Answers how many were deleted.
   */
  memory_notes_discard_local: cmd<{ ids: LocalNoteIds; scope?: ScriptScope }, number>(),
  /**
   * Dry run of copying everything saved on this computer, all repositories, to the remote database: `kind` is the notes (`harness-memory push
   * --dry-run`) or the scripts (`scripts push --dry-run`), never with `--scope`. The server's plan is kept under `previewId`, one per kind.
   * Can take long. `unsupported` when the server predates it.
   */
  memory_notes_copy_preview: cmd<{ kind: MemoryCopyKind }, NotesCopyPreview>(),
  /** One page of the plan of `previewId`, grouped by repository: only the `conflicts` or `all` rows (default `all`). `notFound` for an unknown plan. */
  memory_notes_copy_rows: cmd<{ kind: MemoryCopyKind; previewId: string; filter?: NotesCopyFilter; offset: number; limit: number }, NotesCopyRows>(),
  /**
   * Copies by the plan of `previewId`: conflicts are overwritten per `overwrite` (`none`, `all`, or the local ids ticked), then local
   * copies that arrived are deleted. `kept` counts those left on this computer, `leftover` their ids. The plan is used up.
   */
  memory_notes_copy_apply: cmd<{ kind: MemoryCopyKind; previewId: string; overwrite: LocalNoteIds }, NotesCopyResult>(),
  /**
   * Creates a note (`note_save` over `harness-memory mcp`, 15 s), or with `replaceId` rewrites that note (`note_update`: an edit,
   * or replacing the note that has the title, whose scope stays). A same-title global note is rewritten (`updated`). A note of the
   * project's repository with the title, or another note of the edited note's scope, is `exists` with that note's id, and nothing
   * is saved. `id` is null when the server's answer does not say. Rejects with `invalidInput` for a draft over a limit or a new
   * repository note without a project, `notFound` when the note to replace is gone or Harness Memory is not installed,
   * `unsupported` when the server predates saving from the app, `process` with the server's (redacted) message when it refuses,
   * `forbidden` for a project that is not open or recent.
   */
  memory_note_save: cmd<{ project: string | null; draft: NoteDraft; replaceId?: number }, NoteSaved>(),
  /**
   * Deletes every note `memory_notes_list` shows for `project` without filters, every page (`harness-memory notes delete
   * --all --yes`, 60 s): `deleted` counts them, `team` those that were in the team database (null when the server does not
   * say). Rejects with `unsupported` when the server predates it, `notFound` when Harness Memory is not installed,
   * `process` with the server's (redacted) message when it fails, `forbidden` for a project that is not open or recent.
   */
  memory_notes_delete_all: cmd<{ project?: string | null }, MemoryDeleteAll>(),
  /**
   * Opens the native file picker for a CSV or JSON file of notes and reviews it for `project` (null for global notes only). Null when
   * the user cancelled the picker. `plan`: at most 500 notes (`truncated` past that) of a file of at most 2 MiB, each row with what is
   * wrong with it in `problems` (codes, worded by the UI) and `duplicateOf` pointing a later row at the first with its title. Rust keeps
   * the rows under `planId`. `refused`: the file cannot be read as notes, `reason` says why (`line` for `invalidCsv` when known). Either
   * way any earlier review is dropped. Rejects with `forbidden` for a project that is not open or recent or a file on the deny-list,
   * `io` when the file cannot be read.
   */
  memory_notes_import_preview: cmd<{ project: string | null }, NotesImportPreview | null>(),
  /**
   * Imports the reviewed rows `rows` (indexes) of `planId` in the file's order (`note_save`), each with its own scope or `defaultScope`,
   * and sends `onProgress` after each. A note of the row's scope with its title skips the row (`exists`), or with `replace` is
   * rewritten (`replaced`). `updated` counts notes the server rewrote by itself (as `memory_note_save` answers). Rejects with `invalidInput` (nothing saved, the review
   * stays) when `planId` is not the current review or `project` is not the one it was made for. Otherwise the review is used up, also
   * when the call then rejects with `invalidInput` for its rows (none, or one with problems), `notFound` when Harness Memory is not
   * installed or `unsupported` when it predates saving from the app (rows saved before stay saved). `forbidden` for a project that is
   * not open or recent.
   */
  memory_notes_import_apply: cmd<
    { planId: string; rows: number[]; defaultScope: ScriptScope; replace: boolean; project: string | null; onProgress: EventSink<NotesImportProgress> },
    NotesImportResult
  >(),
  /**
   * Opens the native save dialog for an example file of notes in `format` (`notes-example.csv` or `.json`) and writes it where the user
   * chose. The path it was saved to, null when the user cancelled the dialog. Rejects with `io` when the file cannot be written.
   */
  memory_notes_import_template: cmd<{ format: NotesImportFormat }, string | null>(),
};
