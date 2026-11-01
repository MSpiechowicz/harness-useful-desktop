//! The installed `harness-code` MCP server in a harness's MCP file (`register_code_mcp`,
//! `unregister_code_mcp`).
//!
//! What is found, owned and added is [`sidecar_register`](super::sidecar_register)'s, for
//! [`CODE_SIDECAR`] (the entry is named [`CODE_MCP_SERVER`](super::catalog::CODE_MCP_SERVER) and
//! runs `harness-code mcp`).

use std::path::Path;

use super::catalog::CODE_SIDECAR;
use super::mcp_cli::{register_sidecar_mcp_cli, unregister_sidecar_mcp_cli};
use super::mcp_merge::Blocked;
use super::setup::McpFile;
use super::sidecar_register::{SidecarRegistration, register_sidecar_mcp, unregister_sidecar_mcp};
use crate::harness::HarnessCli;

/// What [`register_code_mcp`] did with a file.
pub type CodeRegistration = SidecarRegistration;

/// Registers the code server in `file` as `command mcp` (see [`register_sidecar_mcp`]). `recorded`
/// tells that the app registered it in that file before.
pub fn register_code_mcp(file: &McpFile, command: &Path, recorded: bool) -> CodeRegistration {
    register_sidecar_mcp(&CODE_SIDECAR, file, command, recorded)
}

/// Removes the owned entry from `file` (see [`unregister_sidecar_mcp`]). `Ok(true)` when it was
/// removed.
pub fn unregister_code_mcp(file: &McpFile) -> Result<bool, Blocked> {
    unregister_sidecar_mcp(&CODE_SIDECAR, file)
}

/// [`register_code_mcp`] through the harness's CLI (see [`register_sidecar_mcp_cli`]).
pub async fn register_code_mcp_cli(
    cli: HarnessCli<'_>,
    file: &McpFile,
    command: &Path,
    recorded: bool,
) -> CodeRegistration {
    register_sidecar_mcp_cli(&CODE_SIDECAR, cli, file, command, recorded).await
}

/// [`unregister_code_mcp`] through the harness's CLI (see [`unregister_sidecar_mcp_cli`]).
pub async fn unregister_code_mcp_cli(cli: HarnessCli<'_>, file: &McpFile) -> Result<bool, Blocked> {
    unregister_sidecar_mcp_cli(&CODE_SIDECAR, cli, file).await
}
