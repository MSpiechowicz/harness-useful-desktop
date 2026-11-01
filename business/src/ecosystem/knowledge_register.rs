//! The installed `harness-knowledge` MCP server in a harness's MCP file (`register_knowledge_mcp`,
//! `unregister_knowledge_mcp`).
//!
//! What is found, owned and added is [`sidecar_register`](super::sidecar_register)'s, for
//! [`KNOWLEDGE_SIDECAR`] (the entry is named
//! [`KNOWLEDGE_MCP_SERVER`](super::catalog::KNOWLEDGE_MCP_SERVER) and runs `harness-knowledge mcp`).

use std::path::Path;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::mcp_cli::{register_sidecar_mcp_cli, unregister_sidecar_mcp_cli};
use super::mcp_merge::Blocked;
use super::setup::McpFile;
use super::sidecar_register::{SidecarRegistration, register_sidecar_mcp, unregister_sidecar_mcp};
use crate::harness::HarnessCli;

/// What [`register_knowledge_mcp`] did with a file.
pub type KnowledgeRegistration = SidecarRegistration;

/// Registers the knowledge server in `file` as `command mcp` (see [`register_sidecar_mcp`]).
/// `recorded` tells that the app registered it in that file before.
pub fn register_knowledge_mcp(file: &McpFile, command: &Path, recorded: bool) -> KnowledgeRegistration {
    register_sidecar_mcp(&KNOWLEDGE_SIDECAR, file, command, recorded)
}

/// Removes the owned entry from `file` (see [`unregister_sidecar_mcp`]). `Ok(true)` when it was
/// removed.
pub fn unregister_knowledge_mcp(file: &McpFile) -> Result<bool, Blocked> {
    unregister_sidecar_mcp(&KNOWLEDGE_SIDECAR, file)
}

/// [`register_knowledge_mcp`] through the harness's CLI (see [`register_sidecar_mcp_cli`]).
pub async fn register_knowledge_mcp_cli(
    cli: HarnessCli<'_>,
    file: &McpFile,
    command: &Path,
    recorded: bool,
) -> KnowledgeRegistration {
    register_sidecar_mcp_cli(&KNOWLEDGE_SIDECAR, cli, file, command, recorded).await
}

/// [`unregister_knowledge_mcp`] through the harness's CLI (see [`unregister_sidecar_mcp_cli`]).
pub async fn unregister_knowledge_mcp_cli(cli: HarnessCli<'_>, file: &McpFile) -> Result<bool, Blocked> {
    unregister_sidecar_mcp_cli(&KNOWLEDGE_SIDECAR, cli, file).await
}
