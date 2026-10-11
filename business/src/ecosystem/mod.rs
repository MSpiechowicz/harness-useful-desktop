//! The Harness Useful ecosystem: the companion tools on the Ecosystem tab and the dashboard data
//! shown in the app.
//!
//! - [`catalog`]: names, links and copyable install steps (never run by the app).
//! - [`detect`]: which tools are installed (`ecosystem_status`).
//! - [`mcp_client`]: a minimal MCP stdio client.
//! - [`releases`]: the latest GitHub release of each installed tool (`ecosystem_releases`).
//! - [`dashboard`]: the pooled `harness-dashboard mcp` connection (`dashboard_query`,
//!   `dashboard_availability`).

pub mod catalog;
pub mod dashboard;
pub mod detect;
pub mod mcp_client;
pub mod releases;

pub use catalog::{ECOSYSTEM, catalog_item, supported_on};
pub use dashboard::{Dashboard, DashboardOptions, resolve_dashboard};
pub use detect::ecosystem_status;
pub use mcp_client::{McpClient, McpOptions, ToolOutput};
pub use releases::{CompanionReleases, release_endpoint};
