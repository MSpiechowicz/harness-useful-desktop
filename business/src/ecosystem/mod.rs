//! The Harness Useful ecosystem: the companion tools on the Ecosystem tab and the dashboard data
//! shown in the app.
//!
//! - [`catalog`]: names, links and copyable install steps (not run, except the dashboard's
//!   installer in the setup step).
//! - [`detect`]: which tools are installed (`ecosystem_status`).
//! - [`setup`]: what the "Recommended setup" step offers (`setup_status`).
//! - [`installer`]: runs the dashboard's installer for the setup step, once the user confirms.
//! - [`mcp_register`]: the dashboard's MCP server in a harness's user MCP file.
//! - [`mcp_client`]: a minimal MCP stdio client.
//! - [`releases`]: the latest GitHub release of each installed tool (`ecosystem_releases`).
//! - [`dashboard`]: the pooled `harness-dashboard mcp` connection (`dashboard_query`,
//!   `dashboard_availability`).

pub mod catalog;
pub mod dashboard;
pub mod detect;
pub mod installer;
pub mod mcp_client;
pub mod mcp_register;
pub mod releases;
pub mod setup;

pub use catalog::{ECOSYSTEM, catalog_item, supported_on};
pub use dashboard::{Dashboard, DashboardOptions, resolve_dashboard};
pub use detect::ecosystem_status;
pub use installer::{InstallerSpec, dashboard_installer, run_dashboard_installer, run_dashboard_installer_with};
pub use mcp_client::{McpClient, McpOptions, ToolOutput};
pub use mcp_register::{mcp_state, register_dashboard_mcp};
pub use releases::{CompanionReleases, release_endpoint};
pub use setup::{mcp_target_paths, setup_status};
