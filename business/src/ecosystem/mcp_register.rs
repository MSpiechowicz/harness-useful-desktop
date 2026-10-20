//! The `harness-dashboard` MCP server in a harness's MCP file (`mcp_state`,
//! `register_dashboard_mcp`), and the state of the `harness-memory` one (`memory_mcp_state`; it is
//! registered by [`super::memory_register`]).
//!
//! - Found: a server named [`DASHBOARD_MCP_SERVER`], or any server that runs `harness-dashboard`
//!   (any directory, `.exe` or not) with the single argument `mcp`, as `"command"` and `"args"`
//!   (`mcpServers`) or as a `"command"` array (OpenCode), in the target file or one of the
//!   `also_read` files (first file wins). It is disabled when the target file's `disabledServers`
//!   names it, or it sets `"enabled": false` and `enabledServers` does not name it. The memory
//!   server is found the same way, by [`MEMORY_MCP_SERVER`] and `harness-memory`.
//! - Register: adds the file's [`Shape::entry`] for `harness-dashboard` under
//!   [`change_file`](super::mcp_merge::change_file)'s rules: other keys, servers and values kept in
//!   order, a missing file created (mode `0600`), a blank file counted as `{}`, the same refusals,
//!   an atomic write keeping permissions, one re-read when the file changed meanwhile. Nothing is
//!   written when the server is found.
//! - Manual edit: a JSONC file with comments is never written; when the server is not in it, the
//!   state is [`McpState::ManualEdit`] with the entry to paste.
//!
//! File contents are never logged or returned; states name the file by its home-relative path.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::catalog::{DASHBOARD_BINARY, DASHBOARD_MCP_SERVER, MEMORY_BINARY, MEMORY_MCP_SERVER};
use super::mcp_merge::{Blocked, Change, Shape, Target, change_file, load, servers, servers_mut};
use super::memory_binary::resolve_memory;
use super::setup::McpFile;
use crate::env::Env;
use crate::model::{McpRefusal, McpState};
use crate::pi_family::{string_list, tilde};

/// The dashboard's server in `file`, read-only (see the module docs).
pub fn mcp_state(env: &Env, file: &McpFile) -> McpState {
    server_state(env, file, DASHBOARD)
}

/// The memory server in `file`, read-only (see the module docs). A manual edit names the
/// `harness-memory` the app would register.
pub fn memory_mcp_state(env: &Env, file: &McpFile) -> McpState {
    server_state(env, file, MEMORY)
}

/// A companion's MCP server: the entry name the app writes, and the binary it runs.
#[derive(Clone, Copy)]
struct Server {
    name: &'static str,
    binary: &'static str,
    /// Whether the app's entry runs the binary by its resolved path rather than by name.
    by_path: bool,
}

impl Server {
    /// The command the app's entry runs (the binary's name when it does not resolve).
    fn command(self, env: &Env) -> String {
        let resolved = if self.by_path { resolve_memory(env) } else { None };

        match resolved {
            Some(binary) => binary.path.to_string_lossy().into_owned(),
            None => self.binary.to_owned(),
        }
    }
}

const DASHBOARD: Server = Server {
    name: DASHBOARD_MCP_SERVER,
    binary: DASHBOARD_BINARY,
    by_path: false,
};

const MEMORY: Server = Server {
    name: MEMORY_MCP_SERVER,
    binary: MEMORY_BINARY,
    by_path: true,
};

fn server_state(env: &Env, file: &McpFile, server: Server) -> McpState {
    let target = match load(&file.path, file.shape) {
        Ok(target) => target,
        Err(reason) => return refused(env, &file.path, reason),
    };

    if let Some(state) = found(env, file, &target, server) {
        return state;
    }

    if !target.rewritable() {
        let entry = file.shape.entry(&server.command(env));
        return manual_edit(env, &file.path, server.name, &entry);
    }

    McpState::Missing {
        file: tilde(env, &file.path),
        exists: matches!(target, Target::Present { .. }),
    }
}

/// Registers the dashboard's server in `file` unless it is already there or the file is refused
/// (see the module docs), and returns the resulting state.
pub fn register_dashboard_mcp(env: &Env, file: &McpFile) -> McpState {
    let entry = file.shape.entry(DASHBOARD_BINARY);

    // Every read of the file is checked for the server first, so an entry that appeared meanwhile
    // (under any name) is reported rather than doubled.
    let changed = change_file(&file.path, file.shape, |target| {
        if let Some(state) = found(env, file, target, DASHBOARD) {
            return Change::Keep(state);
        }

        let mut document = target.document();
        add_server(&mut document, file.shape, DASHBOARD_MCP_SERVER, entry.clone());

        let configured = McpState::Configured {
            file: tilde(env, &file.path),
            name: DASHBOARD_MCP_SERVER.to_owned(),
        };
        Change::Write(document, configured)
    });

    match changed {
        Ok(state) => state,
        Err(Blocked::Refused(reason)) => refused(env, &file.path, reason),
        Err(Blocked::ManualEdit) => manual_edit(env, &file.path, DASHBOARD_MCP_SERVER, &entry),
    }
}

/// Adds `definition` as the server `name` of `document`, unless a server has that name.
pub(crate) fn add_server(document: &mut Map<String, Value>, shape: Shape, name: &str, definition: Value) {
    // `load` refuses a servers key that is not an object, so this always finds one.
    if let Some(servers) = servers_mut(document, shape) {
        servers.entry(name).or_insert(definition);
    }
}

/// `server` in the target or an `also_read` file, as `Configured` or `Disabled`.
fn found(env: &Env, file: &McpFile, target: &Target, server: Server) -> Option<McpState> {
    let server = find_server(file, target, server.name, server.binary)?;
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

/// The server named `name`, else the first one running `<binary> mcp`, in the target file (read as
/// `target`) or one of the `also_read` files (the first file holding one wins).
pub(crate) fn find_server(file: &McpFile, target: &Target, name: &str, binary: &str) -> Option<FoundServer> {
    let target_document = target.document();

    let disabled = string_list(target_document.get("disabledServers"));
    let forced_on = string_list(target_document.get("enabledServers"));

    let others = file.also_read.iter().filter_map(|other| match load(other, file.shape) {
        Ok(Target::Present { document, .. }) => Some((other.as_path(), document)),
        _ => None,
    });
    let mut documents = std::iter::once((file.path.as_path(), target_document.clone())).chain(others);

    documents.find_map(|(path, document)| {
        let servers = servers(&document, file.shape)?;
        let (found_name, server) = server_entry(servers, name, binary)?;

        let switched_off = server.get("enabled") == Some(&Value::Bool(false)) && !forced_on.contains(found_name);

        Some(FoundServer {
            file: path.to_path_buf(),
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

/// True when `server` runs `binary` (any directory, `.exe` or not, any case) with the single
/// argument `mcp`, in either shape.
pub(crate) fn runs_mcp(server: &Value, binary: &str) -> bool {
    let Some((program, args)) = command_line(server) else {
        return false;
    };

    let file_name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);

    stem == binary && args.len() == 1 && args[0] == "mcp"
}

/// The program `server` runs and its arguments: `"command"` and `"args"` (`mcpServers`), or a
/// `"command"` array (OpenCode).
fn command_line(server: &Value) -> Option<(&str, &[Value])> {
    match server.get("command")? {
        Value::String(program) => {
            let args = server.get("args").and_then(Value::as_array)?;
            Some((program, args))
        }
        Value::Array(parts) => {
            let (program, args) = parts.split_first()?;
            Some((program.as_str()?, args))
        }
        _ => None,
    }
}

/// The program `server` runs, in either shape.
pub(crate) fn server_program(server: &Value) -> Option<&str> {
    match server.get("command")? {
        Value::String(program) => Some(program),
        Value::Array(parts) => parts.first()?.as_str(),
        _ => None,
    }
}

/// The state that asks the user to add `entry` as `name` to `path` themselves.
fn manual_edit(env: &Env, path: &Path, name: &str, entry: &Value) -> McpState {
    McpState::ManualEdit {
        file: tilde(env, path),
        snippet: snippet(name, entry),
    }
}

/// `"<name>": <entry>`, pretty-printed, ready to paste into the servers object.
pub(crate) fn snippet(name: &str, entry: &Value) -> String {
    let name = Value::String(name.to_owned());
    let entry = serde_json::to_string_pretty(entry).unwrap_or_default();

    format!("{name}: {entry}")
}

fn refused(env: &Env, path: &Path, reason: McpRefusal) -> McpState {
    McpState::Refused {
        file: tilde(env, path),
        reason,
    }
}
