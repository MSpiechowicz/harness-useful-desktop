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
//! - [`mcp_client`]: a minimal MCP stdio client.
//! - [`memory_remote`]: the shared team database in Harness Memory's own config file, and
//!   `harness-memory check` against it.
//! - [`memory_recall`]: the notes `harness-memory` finds for a chat message, put in front of it.
//! - [`releases`]: the latest GitHub release of each installed tool (`ecosystem_releases`).
//! - [`dashboard`]: the pooled `harness-dashboard mcp` connection (`dashboard_query`,
//!   `dashboard_availability`).

pub mod catalog;
pub mod dashboard;
pub mod detect;
pub mod installer;
pub mod mcp_client;
pub mod mcp_merge;
pub mod mcp_register;
pub mod memory_binary;
pub mod memory_migrate;
pub mod memory_recall;
pub mod memory_record;
pub mod memory_register;
pub mod memory_remote;
mod recall_text;
pub mod releases;
pub mod setup;

pub use catalog::{ECOSYSTEM, catalog_item, supported_on};
pub use dashboard::{Dashboard, DashboardOptions, resolve_dashboard};
pub use detect::ecosystem_status;
pub use installer::{Companion, InstallerSpec, installer, run_installer, run_installer_with};
pub use mcp_client::{McpClient, McpOptions, ToolOutput};
pub use mcp_merge::{Blocked, MergeOutcome, MergeResult, MergedServer, Shape, existing_server, merge_servers};
pub use mcp_register::{mcp_state, memory_mcp_state, register_dashboard_mcp};
pub use memory_binary::{installed_memory, memory_version, parse_memory_version, resolve_memory};
pub use memory_migrate::{LegacyCopy, LegacyMigration, migrate_legacy_memory};
pub use memory_recall::{MemoryRecall, RecallOutcome, RecallReason, WarmRecall};
pub use memory_record::{MEMORY_RECORD_FILE, MemoryRecord, RecordedFile};
pub use memory_register::{
    MemoryRegistration, names_command, register_memory_mcp, repoint_memory_mcp, unregister_memory_mcp,
};
pub use memory_remote::{
    MemoryRemoteChange, memory_config_path, memory_remote_get, memory_remote_set, memory_remote_test,
};
pub use releases::{CompanionReleases, release_endpoint};
pub use setup::{McpFile, mcp_target, mcp_target_paths, memory_server_setup, setup_status};
