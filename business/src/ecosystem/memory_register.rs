//! The installed `harness-memory` MCP server in a harness's MCP file (`register_memory_mcp`,
//! `unregister_memory_mcp`, and `repoint_memory_mcp` for the old copy, see
//! [`super::memory_migrate`]).
//!
//! What is found, owned and added is [`sidecar_register`](super::sidecar_register)'s, for
//! [`MEMORY_SIDECAR`] (the entry is named [`MEMORY_MCP_SERVER`](super::catalog::MEMORY_MCP_SERVER)
//! and runs `harness-memory mcp`).

use std::path::Path;

use super::catalog::MEMORY_SIDECAR;
use super::mcp_cli::{register_sidecar_mcp_cli, unregister_sidecar_mcp_cli};
use super::mcp_merge::Blocked;
use super::setup::McpFile;
use super::sidecar_register::{SidecarRegistration, register_sidecar_mcp, repoint_sidecar_mcp, unregister_sidecar_mcp};
use crate::harness::HarnessCli;
use crate::model::McpRefusal;

pub use super::sidecar_register::names_command;

/// What [`register_memory_mcp`] did with a file.
pub type MemoryRegistration = SidecarRegistration;

/// Registers the memory server in `file` as `command mcp` (see [`register_sidecar_mcp`]).
/// `recorded` tells that the app registered it in that file before.
pub fn register_memory_mcp(file: &McpFile, command: &Path, recorded: bool) -> MemoryRegistration {
    register_sidecar_mcp(&MEMORY_SIDECAR, file, command, recorded)
}

/// Removes the owned entry from `file` (see [`unregister_sidecar_mcp`]). `Ok(true)` when it was
/// removed.
pub fn unregister_memory_mcp(file: &McpFile) -> Result<bool, Blocked> {
    unregister_sidecar_mcp(&MEMORY_SIDECAR, file)
}

/// Changes the `command` of the owned entry in the `mcpServers` file `path` from exactly `from` to
/// `to` (see [`repoint_sidecar_mcp`]). `Ok(true)` when it was changed.
pub fn repoint_memory_mcp(path: &Path, from: &Path, to: &Path) -> Result<bool, McpRefusal> {
    repoint_sidecar_mcp(&MEMORY_SIDECAR, path, from, to)
}

/// [`register_memory_mcp`] through the harness's CLI (see [`register_sidecar_mcp_cli`]).
pub async fn register_memory_mcp_cli(
    cli: HarnessCli<'_>,
    file: &McpFile,
    command: &Path,
    recorded: bool,
) -> MemoryRegistration {
    register_sidecar_mcp_cli(&MEMORY_SIDECAR, cli, file, command, recorded).await
}

/// [`unregister_memory_mcp`] through the harness's CLI (see [`unregister_sidecar_mcp_cli`]).
pub async fn unregister_memory_mcp_cli(cli: HarnessCli<'_>, file: &McpFile) -> Result<bool, Blocked> {
    unregister_sidecar_mcp_cli(&MEMORY_SIDECAR, cli, file).await
}
