//! The Harness Useful ecosystem: the companion tools on the Ecosystem tab and the dashboard data
//! shown in the app.
//!
//! - [`catalog`]: names, links and copyable install steps (not run, except the dashboard's, the
//!   memory server's and the Harness Skills' OpenCode and Codex installers).
//! - [`detect`]: which tools are installed (`ecosystem_status`).
//! - [`setup`]: what the "Recommended setup" step offers (`setup_status`).
//! - [`installer`]: runs the dashboard's, the memory server's or the Harness Skills' OpenCode or
//!   Codex installer, once the user confirms, and [`skills_record`]: the install record that tells
//!   whether the Harness Skills for OpenCode or Codex are installed.
//! - [`mcp_merge`]: adds server entries to a harness's user MCP file without overwriting any, and
//!   [`mcp_cli`]: the companion servers in a file only the harness's CLI changes (Claude Code).
//! - [`mcp_register`]: the dashboard's MCP server in a harness's user MCP file, and the state of
//!   the memory server's.
//! - [`memory_binary`]: the installed `harness-memory` server and its version.
//! - [`memory_register`]: that server in a harness's user MCP file, [`memory_record`]: the files
//!   the app registered it in, and [`memory_migrate`]: the old copy of the bundled server retired.
//! - [`desktop_register`]: the app's own MCP server (`harness-desktop mcp`, hooks for agents) in a
//!   harness's user MCP file, and [`desktop_record`]: the files the app registered it in.
//! - [`mcp_client`]: a minimal MCP stdio client, and [`mcp_pool`]: one such client kept running
//!   between requests.
//! - [`memory_remote`]: the shared team database in Harness Memory's own config file, and
//!   `harness-memory check` against it.
//! - [`releases`]: the latest GitHub release of each installed tool (`ecosystem_releases`).
//! - [`dashboard`]: the pooled `harness-dashboard mcp` connection (`dashboard_query`,
//!   `dashboard_availability`).
//! - [`memory_cli`]: one run of a `harness-memory` command for the user, and the page of a list.
//! - [`memory_notes`]: the agents' notes through `harness-memory notes` (`memory_notes_list`,
//!   `memory_note_get`, `memory_note_delete`, `memory_notes_delete_all`), [`memory_notes_save`]:
//!   one created or edited through `harness-memory mcp` (`memory_note_save`), and
//!   [`memory_notes_import`]: notes imported from a CSV or JSON file after a review
//!   (`memory_notes_import_preview`, `memory_notes_import_apply`), with an example file to fill in
//!   (`memory_notes_import_template`).
//! - [`memory_scripts`]: the agents' scripts, listed with `harness-memory scripts list` and read
//!   through the pooled `harness-memory mcp` connection (`memory_scripts_list`,
//!   `memory_script_get`, `memory_script_delete`, `memory_scripts_delete_all`),
//!   [`memory_scripts_save`]: one created or edited there (`memory_script_save`), [`memory_requests`]:
//!   the server's run requests for scripts this machine has not approved, the heartbeat that
//!   lets it ask, and the answers, and [`memory_trust`]: `harness-memory trust`
//!   (`memory_scripts_trust`). [`memory_hidden`]: a team note or script hidden on this machine
//!   (`memory_note_hide`, `memory_script_hide`) and local copies discarded
//!   (`memory_notes_discard_local`, `memory_scripts_discard_local`).
//! - [`memory_team_docs`]: team documents of a signed document kind Harness Memory keeps
//!   (`harness-memory <plural> …`), and whether the installed server has them; [`memory_hooks`]:
//!   the same for hooks (`harness-memory hooks …`).
//! - [`memory_kinds`]: which team databases the installed server can use (`harness-memory
//!   kinds`), [`memory_features`]: what else it can do (`harness-memory features`), both probed
//!   through [`memory_line_probe`].
//! - [`memory_guards`]: the save guards of every chat and of each one (`memory_guards_get`,
//!   `memory_guards_update`, `memory_chat_guards_set`), [`memory_held`]: the saves held for the user's
//!   review (`memory_held_list`, `memory_held_save`, `memory_held_skip`), and
//!   [`memory_user_save`]: the user's own saves through commands agents cannot reach.
//! - [`memory_notes_copy`]: copying the notes or scripts saved on this computer to the team database
//!   (`harness-memory push --json`, `scripts push --json`), its answer read by `memory_copy_answer`.

pub mod catalog;
pub mod dashboard;
pub mod desktop_record;
pub mod desktop_register;
pub mod detect;
pub mod installer;
mod installer_log;
pub mod mcp_cli;
pub mod mcp_client;
pub mod mcp_merge;
pub mod mcp_pool;
pub mod mcp_register;
pub mod mcp_rename;
pub mod memory_binary;
pub mod memory_cli;
mod memory_copy_answer;
pub mod memory_embedder;
pub mod memory_embedder_supervisor;
pub mod memory_features;
pub mod memory_guards;
pub mod memory_held;
pub mod memory_hidden;
pub mod memory_hooks;
pub mod memory_kinds;
mod memory_line_probe;
mod memory_list;
pub mod memory_migrate;
pub mod memory_notes;
pub mod memory_notes_copy;
pub mod memory_notes_import;
pub mod memory_notes_save;
mod memory_notes_titles;
pub mod memory_record;
pub mod memory_register;
pub mod memory_remote;
pub mod memory_repos;
pub mod memory_requests;
pub mod memory_scripts;
mod memory_scripts_list;
pub mod memory_scripts_run;
pub mod memory_scripts_save;
pub mod memory_team_docs;
pub mod memory_trust;
pub mod memory_user_save;
pub mod releases;
pub mod setup;
pub mod skills_migration;
pub mod skills_paths;
pub mod skills_record;

pub use catalog::{ECOSYSTEM, catalog_item, supported_on};
pub use dashboard::{Dashboard, DashboardOptions, resolve_dashboard};
pub use desktop_record::{DESKTOP_RECORD_FILE, DesktopRecord, DesktopRecordEntry};
pub use desktop_register::{
    DESKTOP_MCP_SERVER, DesktopRegistration, app_command, current_app_command, desktop_mcp_state, register_desktop_mcp,
};
pub use detect::ecosystem_status;
pub use installer::{Companion, Installed, InstallerSpec, installer, run_installer, run_installer_with};
pub use mcp_cli::{
    MCP_CLI_TIMEOUT, register_dashboard_mcp_cli, register_desktop_mcp_cli, register_memory_mcp_cli,
    unregister_memory_mcp_cli,
};
pub use mcp_client::{McpClient, McpOptions, McpTool, ToolOutput};
pub use mcp_merge::{Blocked, MergeOutcome, MergeResult, MergedServer, Shape, existing_server, merge_servers};
pub use mcp_pool::{McpPool, PoolOptions};
pub use mcp_register::{mcp_state, memory_mcp_state, register_dashboard_mcp};
pub use mcp_rename::rename_dashboard_mcp_file;
pub use memory_binary::{installed_memory, memory_version, parse_memory_version, resolve_memory};
pub use memory_cli::ListWindow;
pub use memory_cli::Repos;
pub use memory_embedder::{
    EmbedderEvent, EmbedderStatus, EmbedderTracker, embedder_status, feature_server, get_settings, parse_event,
    remove_embedder_model, set_embedder_enabled, set_settings,
};
pub use memory_embedder_supervisor::{EmbedderSupervisor, SupervisorOptions, embedder_spec};
pub use memory_features::{FeaturesSupport, MemoryFeatures};
pub use memory_guards::{
    CHAT_MAX_AGE, ChatGuardWrites, SESSION_VAR, chat_repo_root, check_chat_key, is_chat_key, read_chat, read_chat_repo,
    read_global, sweep_chats, update_global, write_chat,
};
pub use memory_held::{HeldSave, guarded_server, held_stamp, list_held, save_held, skip_held};
pub use memory_hidden::{MAX_LOCAL_IDS, discard_local, hide_entry};
pub use memory_hooks::{HooksConfig, HooksSupport, MemoryHooks};
pub use memory_kinds::{KindsSupport, MemoryKinds};
pub use memory_migrate::{LegacyCopy, LegacyMigration, migrate_legacy_memory};
pub use memory_notes::{NotesQuery, delete_all_notes, delete_note, get_note, list_notes};
pub use memory_notes_copy::{
    APPLY_TIMEOUT, NotesCopyCache, PREVIEW_TIMEOUT, PUSH_OUTPUT_BYTES, apply_notes_copy, preview_notes_copy,
};
pub use memory_notes_import::{
    FileRefusal, MAX_IMPORT_BYTES, MAX_IMPORT_ROWS, NotesImportRead, NotesImportRequest, StoredNotesImport,
    apply_notes_import, import_template, read_notes_file,
};
pub use memory_notes_save::save_note;
pub use memory_record::{MEMORY_RECORD_FILE, MemoryRecord, RecordedFile};
pub use memory_register::{
    MemoryRegistration, names_command, register_memory_mcp, repoint_memory_mcp, unregister_memory_mcp,
};
pub use memory_remote::{
    MemoryRemoteChange, MemorySupport, ScriptsSupport, memory_config_path, memory_remote_get, memory_remote_set,
    memory_remote_test,
};
pub use memory_repos::list_repos;
pub use memory_requests::{RequestWatcher, memory_data_dir, remove_alive, touch_alive};
pub use memory_scripts::{MemoryScripts, SCRIPT_LIST_TOOL, ScriptsQuery};
pub use memory_team_docs::{MemoryTeamDocs, TeamDocsConfig, TeamDocsSupport};
pub use memory_trust::{script_sha256, trust_scripts};
pub use memory_user_save::SaveRoute;
pub use releases::{CompanionReleases, release_endpoint};
pub use setup::{
    LegacySkillsStep, McpFile, legacy_skills_step, mcp_target, mcp_target_paths, memory_server_setup, setup_status,
    share_mcp_target,
};
pub use skills_migration::{MIGRATION_RECORD_FILE, MigrationExecutor, MigrationRecord, migrate_skills_names};
pub use skills_paths::skills_data_path;
pub use skills_record::{
    SkillsRecord, legacy_skills_record, skills_install, skills_installed, skills_record, skills_version,
};
