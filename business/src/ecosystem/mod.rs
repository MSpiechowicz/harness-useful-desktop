//! The Harness Useful ecosystem: the companion tools on the Ecosystem tab and the dashboard data
//! shown in the app.
//!
//! - [`catalog`]: names, links and copyable install steps (not run, except the dashboard's and
//!   the memory server's installers).
//! - [`detect`]: which tools are installed (`ecosystem_status`).
//! - [`setup`]: what the "Recommended setup" step offers (`setup_status`).
//! - [`installer`]: runs the dashboard's or the memory server's installer, once the user confirms.
//! - [`mcp_merge`]: adds server entries to a harness's user MCP file without overwriting any.
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
//!   `memory_note_get`, `memory_note_delete`, `memory_notes_delete_all`).
//! - [`memory_scripts`]: the agents' scripts, listed with `harness-memory scripts list` and read
//!   through the pooled `harness-memory mcp` connection (`memory_scripts_list`,
//!   `memory_script_get`, `memory_script_delete`, `memory_scripts_delete_all`), [`memory_requests`]:
//!   the server's run requests for scripts this machine has not approved, the heartbeat that
//!   lets it ask, and the answers, and [`memory_trust`]: `harness-memory trust`
//!   (`memory_scripts_trust`).

pub mod catalog;
pub mod dashboard;
pub mod desktop_record;
pub mod desktop_register;
pub mod detect;
pub mod installer;
pub mod mcp_client;
pub mod mcp_merge;
pub mod mcp_pool;
pub mod mcp_register;
pub mod memory_binary;
pub mod memory_cli;
pub mod memory_migrate;
pub mod memory_notes;
pub mod memory_record;
pub mod memory_register;
pub mod memory_remote;
pub mod memory_requests;
pub mod memory_scripts;
mod memory_scripts_list;
pub mod memory_trust;
pub mod releases;
pub mod setup;

pub use catalog::{ECOSYSTEM, catalog_item, supported_on};
pub use dashboard::{Dashboard, DashboardOptions, resolve_dashboard};
pub use desktop_record::{DESKTOP_RECORD_FILE, DesktopRecord, DesktopRecordEntry};
pub use desktop_register::{
    DESKTOP_MCP_SERVER, DesktopRegistration, app_command, current_app_command, desktop_mcp_state, register_desktop_mcp,
};
pub use detect::ecosystem_status;
pub use installer::{Companion, InstallerSpec, installer, run_installer, run_installer_with};
pub use mcp_client::{McpClient, McpOptions, McpTool, ToolOutput};
pub use mcp_merge::{Blocked, MergeOutcome, MergeResult, MergedServer, Shape, existing_server, merge_servers};
pub use mcp_pool::{McpPool, PoolOptions};
pub use mcp_register::{mcp_state, memory_mcp_state, register_dashboard_mcp};
pub use memory_binary::{installed_memory, memory_version, parse_memory_version, resolve_memory};
pub use memory_cli::ListWindow;
pub use memory_migrate::{LegacyCopy, LegacyMigration, migrate_legacy_memory};
pub use memory_notes::{NotesQuery, delete_all_notes, delete_note, get_note, list_notes};
pub use memory_record::{MEMORY_RECORD_FILE, MemoryRecord, RecordedFile};
pub use memory_register::{
    MemoryRegistration, names_command, register_memory_mcp, repoint_memory_mcp, unregister_memory_mcp,
};
pub use memory_remote::{
    MemoryRemoteChange, ScriptsSupport, memory_config_path, memory_remote_get, memory_remote_set, memory_remote_test,
};
pub use memory_requests::{RequestWatcher, memory_data_dir, remove_alive, touch_alive};
pub use memory_scripts::{MemoryScripts, SCRIPT_LIST_TOOL, ScriptsQuery};
pub use memory_trust::trust_scripts;
pub use releases::{CompanionReleases, release_endpoint};
pub use setup::{McpFile, mcp_target, mcp_target_paths, memory_server_setup, setup_status};
