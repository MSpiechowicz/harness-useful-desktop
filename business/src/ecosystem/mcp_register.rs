//! The `harness-dashboard` MCP server in a harness's user MCP file (`mcp_state`,
//! `register_dashboard_mcp`), and the state of the `harness-memory` one (`memory_mcp_state`; it is
//! registered by [`super::memory_register`]).
//!
//! - Found: a server named [`DASHBOARD_MCP_SERVER`], or any server whose `command` is
//!   `harness-dashboard` (any directory, `.exe` or not) with the arguments `["mcp"]`, in the target
//!   file or one of the `also_read` files (first file wins). It is disabled when the target file's
//!   `disabledServers` names it, or it sets `"enabled": false` and `enabledServers` does not name it.
//!   The memory server is found the same way, by [`MEMORY_MCP_SERVER`] and `harness-memory`.
//! - Register: merges `{"command": "harness-dashboard", "args": ["mcp"]}` into `mcpServers` under
//!   [`merge_servers`](super::mcp_merge::merge_servers)'s rules: other keys, servers and values
//!   kept in order, a missing file created (mode `0600`), a blank file counted as `{}`, the same
//!   refusals, an atomic write keeping permissions, one re-read when the file changed meanwhile.
//!   Nothing is written when the server is found.
//!
//! File contents are never logged or returned; states name the file by its home-relative path.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::catalog::{DASHBOARD_BINARY, DASHBOARD_MCP_SERVER, MEMORY_BINARY, MEMORY_MCP_SERVER};
use super::mcp_merge::{MergeOutcome, SERVERS, Target, load, merge_servers_unless};
use crate::env::Env;
use crate::model::{McpRefusal, McpState};
use crate::pi_family::{string_list, tilde};

/// The dashboard's server in `path`, read-only (see the module docs).
pub fn mcp_state(env: &Env, path: &Path, also_read: &[PathBuf]) -> McpState {
    server_state(env, path, also_read, DASHBOARD)
}

/// The memory server in `path`, read-only (see the module docs).
pub fn memory_mcp_state(env: &Env, path: &Path, also_read: &[PathBuf]) -> McpState {
    server_state(env, path, also_read, MEMORY)
}

/// A companion's MCP server: the entry name the app writes, and the binary it runs.
#[derive(Clone, Copy)]
struct Server {
    name: &'static str,
    binary: &'static str,
}

const DASHBOARD: Server = Server {
    name: DASHBOARD_MCP_SERVER,
    binary: DASHBOARD_BINARY,
};

const MEMORY: Server = Server {
    name: MEMORY_MCP_SERVER,
    binary: MEMORY_BINARY,
};

fn server_state(env: &Env, path: &Path, also_read: &[PathBuf], server: Server) -> McpState {
    match load(path) {
        Ok(target) => found(env, path, &target, also_read, server).unwrap_or_else(|| missing(env, path, &target)),
        Err(reason) => refused(env, path, reason),
    }
}

/// Registers the dashboard's server in `path` unless it is already there or the file is refused
/// (see the module docs), and returns the resulting state.
pub fn register_dashboard_mcp(env: &Env, path: &Path, also_read: &[PathBuf]) -> McpState {
    let entry = [(
        DASHBOARD_MCP_SERVER.to_owned(),
        json!({ "command": DASHBOARD_BINARY, "args": ["mcp"] }),
    )];

    // Every read of the file is checked for the server first, so an entry that appeared meanwhile
    // (under any name) is reported rather than doubled.
    let merged = merge_servers_unless(path, &entry, |target| found(env, path, target, also_read, DASHBOARD));

    match merged {
        Err(state) => state,
        Ok(MergeOutcome::Refused(reason)) => refused(env, path, reason),
        Ok(MergeOutcome::Merged(_)) => McpState::Configured {
            file: tilde(env, path),
            name: DASHBOARD_MCP_SERVER.to_owned(),
        },
    }
}

/// `server` in the target or an `also_read` file, as `Configured` or `Disabled`.
fn found(env: &Env, path: &Path, target: &Target, also_read: &[PathBuf], server: Server) -> Option<McpState> {
    let server = find_server(path, target, also_read, server.name, server.binary)?;
    let file = tilde(env, &server.file);

    if server.disabled {
        Some(McpState::Disabled {
            file,
            name: server.name,
        })
    } else {
        Some(McpState::Configured {
            file,
            name: server.name,
        })
    }
}

/// A server entry found by [`find_server`].
pub(crate) struct FoundServer {
    /// The file holding it.
    pub file: PathBuf,
    pub name: String,
    pub definition: Value,
    /// Whether the target file switches it off (see the module docs).
    pub disabled: bool,
}

/// The server named `name`, else the first one running `<binary> mcp`, in the target file or one
/// of the `also_read` files (the first file holding one wins).
pub(crate) fn find_server(
    path: &Path,
    target: &Target,
    also_read: &[PathBuf],
    name: &str,
    binary: &str,
) -> Option<FoundServer> {
    let target_document = target.document();

    let disabled = string_list(target_document.get("disabledServers"));
    let forced_on = string_list(target_document.get("enabledServers"));

    let others = also_read.iter().filter_map(|other| match load(other) {
        Ok(Target::Present { document, .. }) => Some((other.as_path(), document)),
        _ => None,
    });
    let mut documents = std::iter::once((path, target_document.clone())).chain(others);

    documents.find_map(|(file, document)| {
        let servers = document.get(SERVERS).and_then(Value::as_object)?;
        let (found_name, server) = server_entry(servers, name, binary)?;

        let switched_off = server.get("enabled") == Some(&Value::Bool(false)) && !forced_on.contains(found_name);

        Some(FoundServer {
            file: file.to_path_buf(),
            name: found_name.clone(),
            definition: server.clone(),
            disabled: disabled.contains(found_name) || switched_off,
        })
    })
}

/// The entry named `name`, else the first one running `<binary> mcp`.
fn server_entry<'a>(servers: &'a Map<String, Value>, name: &str, binary: &str) -> Option<(&'a String, &'a Value)> {
    servers
        .iter()
        .find(|(entry_name, _)| entry_name.as_str() == name)
        .or_else(|| servers.iter().find(|(_, server)| runs_mcp(server, binary)))
}

/// True when `server` runs `binary` (any directory, `.exe` or not, any case) with the arguments
/// `["mcp"]`.
pub(crate) fn runs_mcp(server: &Value, binary: &str) -> bool {
    let Some(command) = server.get("command").and_then(Value::as_str) else {
        return false;
    };

    let file_name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    let args = server.get("args").and_then(Value::as_array);

    stem == binary && args.is_some_and(|args| args.len() == 1 && args[0] == "mcp")
}

fn missing(env: &Env, path: &Path, target: &Target) -> McpState {
    McpState::Missing {
        file: tilde(env, path),
        exists: matches!(target, Target::Present { .. }),
    }
}

fn refused(env: &Env, path: &Path, reason: McpRefusal) -> McpState {
    McpState::Refused {
        file: tilde(env, path),
        reason,
    }
}
