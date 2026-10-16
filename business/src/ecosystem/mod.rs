//! The Harness Useful ecosystem: the companion tools on the Ecosystem tab and the dashboard data
//! shown in the app.
//!
//! - [`catalog`]: names, links and copyable install steps (not run, except the dashboard's
//!   installer in the setup step).
//! - [`detect`]: which tools are installed (`ecosystem_status`).
//! - [`setup`]: what the "Recommended setup" step offers (`setup_status`).
//! - [`installer`]: runs the dashboard's installer for the setup step, once the user confirms.
//! - [`mcp_merge`]: adds server entries to a harness's user MCP file without overwriting any.
//! - [`mcp_register`]: the dashboard's MCP server in a harness's user MCP file.
//! - [`memory_install`]: the app's stable copy of the bundled `harness-memory` server.
//! - [`memory_register`]: that server in a harness's user MCP file, and [`memory_record`]: the
//!   files the app registered it in.
//! - [`mcp_client`]: a minimal MCP stdio client.
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
pub mod memory_install;
pub mod memory_recall;
pub mod memory_record;
pub mod memory_register;
mod recall_text;
pub mod releases;
pub mod setup;

pub use catalog::{ECOSYSTEM, catalog_item, supported_on};
pub use dashboard::{Dashboard, DashboardOptions, resolve_dashboard};
pub use detect::ecosystem_status;
pub use installer::{InstallerSpec, dashboard_installer, run_dashboard_installer, run_dashboard_installer_with};
pub use mcp_client::{McpClient, McpOptions, ToolOutput};
pub use mcp_merge::{MergeOutcome, MergeResult, MergedServer, existing_server, merge_servers};
pub use mcp_register::{mcp_state, register_dashboard_mcp};
pub use memory_install::{InstalledBinary, install_memory_binary, memory_binary_name};
pub use memory_recall::{MemoryRecall, RecallOutcome, RecallReason, WarmRecall};
pub use memory_record::{MEMORY_RECORD_FILE, MemoryRecord, RecordedFile};
pub use memory_register::{MemoryRegistration, register_memory_mcp, unregister_memory_mcp};
pub use releases::{CompanionReleases, release_endpoint};
pub use setup::{mcp_target_paths, setup_status};
