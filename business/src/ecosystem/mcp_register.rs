//! The `harness-dashboard` MCP server in a harness's user MCP file (`mcp_state`,
//! `register_dashboard_mcp`).
//!
//! - Found: a server named [`DASHBOARD_MCP_SERVER`], or any server whose `command` is
//!   `harness-dashboard` (any directory, `.exe` or not) with the arguments `["mcp"]`, in the target
//!   file or one of the `also_read` files (first file wins). It is disabled when the target file's
//!   `disabledServers` names it, or it sets `"enabled": false` and `enabledServers` does not name it.
//! - Register: merges `{"command": "harness-dashboard", "args": ["mcp"]}` into `mcpServers`, keeping
//!   every other key, server and value in order, or creates the file (mode `0600`) holding only
//!   that. A blank file counts as `{}`. Nothing is written when the server is found.
//! - Refused, nothing written: invalid JSON, a root or `mcpServers` that is not an object, a symlink,
//!   anything but a regular file, more than 2 MiB, or a file that cannot be read.
//! - The write is atomic (temporary sibling, flushed, renamed over the file) and keeps the file's
//!   permissions. A file that changed meanwhile is read again once, then refused; a write that
//!   fails (or a second change) is refused as `WriteFailed`, the file left as it was.
//!
//! File contents are never logged or returned; states name the file by its home-relative path.

use std::fs::{self, Metadata, Permissions};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Map, Value, json};

use super::catalog::{DASHBOARD_BINARY, DASHBOARD_MCP_SERVER};
use crate::env::Env;
use crate::error::BusinessError;
use crate::fs_atomic::{temp_sibling, write_synced};
use crate::model::{McpRefusal, McpState};
use crate::pi_family::{string_list, tilde};
use crate::security::SafeReader;

/// Largest MCP file read or changed.
const MAX_MCP_BYTES: u64 = SafeReader::DEFAULT_MAX_BYTES;

/// The key holding the servers.
const SERVERS: &str = "mcpServers";

/// What [`mcp_state`] reports for `path`, read-only (see the module docs).
pub fn mcp_state(env: &Env, path: &Path, also_read: &[PathBuf]) -> McpState {
    match load(path) {
        Ok(target) => found(env, path, &target, also_read).unwrap_or_else(|| missing(env, path, &target)),
        Err(reason) => refused(env, path, reason),
    }
}

/// Registers the dashboard's server in `path` unless it is already there or the file is refused
/// (see the module docs), and returns the resulting state.
pub fn register_dashboard_mcp(env: &Env, path: &Path, also_read: &[PathBuf]) -> McpState {
    for _attempt in 0..2 {
        let target = match load(path) {
            Ok(target) => target,
            Err(reason) => return refused(env, path, reason),
        };

        if let Some(state) = found(env, path, &target, also_read) {
            return state;
        }

        match write(path, target) {
            Ok(()) => {
                return McpState::Configured {
                    file: tilde(env, path),
                    name: DASHBOARD_MCP_SERVER.to_owned(),
                };
            }
            Err(WriteError::Changed) => {}
            Err(WriteError::Failed) => return refused(env, path, McpRefusal::WriteFailed),
        }
    }

    // Changed again after the second read.
    refused(env, path, McpRefusal::WriteFailed)
}

/// The target file as read.
enum Target {
    Absent,
    Present {
        document: Map<String, Value>,
        stamp: Stamp,
        permissions: Permissions,
    },
}

/// What tells that a file changed since it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    is_file: bool,
    len: u64,
    modified: Option<SystemTime>,
}

impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        Self {
            is_file: metadata.file_type().is_file(),
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

fn load(path: &Path) -> Result<Target, McpRefusal> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Target::Absent),
        Err(_) => return Err(McpRefusal::Unreadable),
    };

    if metadata.file_type().is_symlink() {
        return Err(McpRefusal::Symlink);
    }

    if !metadata.is_file() {
        return Err(McpRefusal::NotRegularFile);
    }

    if metadata.len() > MAX_MCP_BYTES {
        return Err(McpRefusal::TooLarge);
    }

    let text = match SafeReader::read_to_string(path, MAX_MCP_BYTES) {
        Ok(text) => text,
        // The checks above passed, so this is invalid UTF-8 (or a file swapped meanwhile).
        Err(BusinessError::InvalidInput(_)) => return Err(McpRefusal::Unparsable),
        Err(_) => return Err(McpRefusal::Unreadable),
    };

    Ok(Target::Present {
        document: parse(&text)?,
        stamp: Stamp::of(&metadata),
        permissions: metadata.permissions(),
    })
}

fn parse(text: &str) -> Result<Map<String, Value>, McpRefusal> {
    if text.trim().is_empty() {
        return Ok(Map::new());
    }

    let value: Value = serde_json::from_str(text).map_err(|_| McpRefusal::Unparsable)?;
    let Value::Object(document) = value else {
        return Err(McpRefusal::NotAnObject);
    };

    match document.get(SERVERS) {
        None | Some(Value::Object(_)) => Ok(document),
        Some(_) => Err(McpRefusal::ServersNotAnObject),
    }
}

/// The dashboard's server in the target or an `also_read` file, as `Configured` or `Disabled`.
fn found(env: &Env, path: &Path, target: &Target, also_read: &[PathBuf]) -> Option<McpState> {
    let empty = Map::new();
    let target_document = match target {
        Target::Present { document, .. } => document,
        Target::Absent => &empty,
    };

    let disabled = string_list(target_document.get("disabledServers"));
    let forced_on = string_list(target_document.get("enabledServers"));

    let others = also_read.iter().filter_map(|other| match load(other) {
        Ok(Target::Present { document, .. }) => Some((other.as_path(), document)),
        _ => None,
    });
    let mut documents = std::iter::once((path, target_document.clone())).chain(others);

    documents.find_map(|(file, document)| {
        let servers = document.get(SERVERS).and_then(Value::as_object)?;
        let (name, server) = dashboard_entry(servers)?;

        let switched_off = server.get("enabled") == Some(&Value::Bool(false)) && !forced_on.contains(name);
        let file = tilde(env, file);
        let name = name.clone();

        if disabled.contains(&name) || switched_off {
            Some(McpState::Disabled { file, name })
        } else {
            Some(McpState::Configured { file, name })
        }
    })
}

/// The entry named [`DASHBOARD_MCP_SERVER`], else the first one running `harness-dashboard mcp`.
fn dashboard_entry(servers: &Map<String, Value>) -> Option<(&String, &Value)> {
    servers
        .iter()
        .find(|(name, _)| name.as_str() == DASHBOARD_MCP_SERVER)
        .or_else(|| servers.iter().find(|(_, server)| runs_dashboard(server)))
}

fn runs_dashboard(server: &Value) -> bool {
    let Some(command) = server.get("command").and_then(Value::as_str) else {
        return false;
    };

    let file_name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    let args = server.get("args").and_then(Value::as_array);

    stem == DASHBOARD_BINARY && args.is_some_and(|args| args.len() == 1 && args[0] == "mcp")
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

/// Why a write did not happen.
enum WriteError {
    /// The file changed after it was read.
    Changed,
    /// Creating, writing or renaming failed.
    Failed,
}

/// Writes `target` plus the dashboard's server atomically over `path`.
fn write(path: &Path, target: Target) -> Result<(), WriteError> {
    let (mut document, original) = match target {
        Target::Absent => (Map::new(), None),
        Target::Present {
            document,
            stamp,
            permissions,
        } => (document, Some((stamp, permissions))),
    };

    let servers = document.entry(SERVERS).or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(servers) = servers else {
        return Err(WriteError::Failed);
    };
    servers.insert(
        DASHBOARD_MCP_SERVER.to_owned(),
        json!({ "command": DASHBOARD_BINARY, "args": ["mcp"] }),
    );

    let mut bytes = serde_json::to_vec_pretty(&Value::Object(document)).map_err(|_| WriteError::Failed)?;
    bytes.push(b'\n');

    if original.is_none()
        && let Some(parent) = path.parent()
    {
        fs::create_dir_all(parent).map_err(|_| WriteError::Failed)?;
    }

    let temp = temp_sibling(path);
    let written = replace(path, &temp, &bytes, original.as_ref());

    if written.is_err() {
        fs::remove_file(&temp).ok();
    }

    written
}

fn replace(path: &Path, temp: &Path, bytes: &[u8], original: Option<&(Stamp, Permissions)>) -> Result<(), WriteError> {
    write_synced(temp, bytes).map_err(|_| WriteError::Failed)?;

    if let Some((_, permissions)) = original {
        fs::set_permissions(temp, permissions.clone()).map_err(|_| WriteError::Failed)?;
    }

    if !unchanged(path, original.map(|(stamp, _)| stamp)) {
        return Err(WriteError::Changed);
    }

    fs::rename(temp, path).map_err(|_| WriteError::Failed)
}

/// True when `path` still matches what was read (`None`: it still does not exist).
fn unchanged(path: &Path, stamp: Option<&Stamp>) -> bool {
    match (fs::symlink_metadata(path), stamp) {
        (Err(error), None) => error.kind() == std::io::ErrorKind::NotFound,
        (Ok(metadata), Some(stamp)) => Stamp::of(&metadata) == *stamp,
        _ => false,
    }
}
