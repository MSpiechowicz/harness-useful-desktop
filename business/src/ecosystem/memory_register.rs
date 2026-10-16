//! The installed `harness-memory` MCP server in a harness's user MCP file (`register_memory_mcp`,
//! `unregister_memory_mcp`, and `repoint_memory_mcp` for the old copy, see
//! [`super::memory_migrate`]).
//!
//! - Found: like the dashboard's server (see [`super::mcp_register`]): a server named
//!   [`MEMORY_MCP_SERVER`], or any server running `harness-memory` (any directory, `.exe` or not)
//!   with the arguments `["mcp"]`, in the target file or one of the `also_read` files, switched off
//!   by the target file's `disabledServers` or by `"enabled": false`.
//! - Owned: an entry in the target file (never an `also_read` one) named exactly
//!   [`MEMORY_MCP_SERVER`] that runs `harness-memory mcp`. Only owned entries are ever changed:
//!   their `command` follows the resolved server, and turning project memory off removes them. Every other field (`enabled`, `env`, ...) and the key order are kept.
//! - Added: `{"command": "<absolute path>", "args": ["mcp"]}`, only when nothing is found and the
//!   app has not registered the server in that file before (a user who removed it keeps it removed).
//!
//! Writes follow [`super::mcp_merge`]'s rules: the same refusals, a new file `0600`, an atomic write
//! keeping permissions, one re-read when the file changed meanwhile. File contents are never logged
//! or returned, except the old and new `command` of an owned entry that is re-pointed.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::catalog::{MEMORY_BINARY, MEMORY_MCP_SERVER};
use super::mcp_merge::{Change, SERVERS, Target, change_file, load};
use super::mcp_register::{FoundServer, find_server, runs_mcp};
use crate::model::McpRefusal;

/// What [`register_memory_mcp`] did with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegistration {
    /// The entry was written.
    Added,
    /// The owned entry named another path; its `command` was changed.
    Updated,
    /// A server running `harness-memory mcp` (or named like it) is there and on.
    Present,
    /// That server is there but switched off; left alone.
    Disabled,
    /// Nothing is there, but the app registered it before: the user removed it, so it stays out.
    SkippedRemoved,
    /// The file cannot be changed safely; nothing was written.
    Refused(McpRefusal),
}

impl MemoryRegistration {
    /// Whether the server is in the file after this registration (enabled or not).
    pub fn is_registered(self) -> bool {
        matches!(self, Self::Added | Self::Updated | Self::Present | Self::Disabled)
    }
}

/// Registers the memory server in `path` as `command mcp` (see the module docs). `recorded` tells
/// that the app registered it in `path` before.
pub fn register_memory_mcp(path: &Path, also_read: &[PathBuf], command: &Path, recorded: bool) -> MemoryRegistration {
    let command = command.to_string_lossy();

    let changed = change_file(path, |target| {
        let Some(server) = find_server(path, target, also_read, MEMORY_MCP_SERVER, MEMORY_BINARY) else {
            return when_missing(target, &command, recorded);
        };

        let previous = server.definition.get("command").and_then(Value::as_str);
        let stale = owned(path, &server) && previous != Some(&command);

        if stale {
            tracing::info!(
                file = %path.display(),
                from = previous.unwrap_or_default(),
                to = %command,
                "project memory: the app's entry now runs another harness-memory"
            );

            let mut document = target.document();
            set_command(&mut document, &command);
            return Change::Write(document, MemoryRegistration::Updated);
        }

        if server.disabled {
            Change::Keep(MemoryRegistration::Disabled)
        } else {
            Change::Keep(MemoryRegistration::Present)
        }
    });

    changed.unwrap_or_else(MemoryRegistration::Refused)
}

/// Removes the owned entry from `path`, leaving every other server and the
/// `disabledServers`/`enabledServers` lists as they are. `Ok(true)` when it was removed.
pub fn unregister_memory_mcp(path: &Path) -> Result<bool, McpRefusal> {
    change_file(path, |target| {
        let found = find_server(path, target, &[], MEMORY_MCP_SERVER, MEMORY_BINARY);

        if !found.is_some_and(|server| owned(path, &server)) {
            return Change::Keep(false);
        }

        let mut document = target.document();
        if let Some(Value::Object(servers)) = document.get_mut(SERVERS) {
            servers.shift_remove(MEMORY_MCP_SERVER);
        }

        Change::Write(document, true)
    })
}

/// Changes the `command` of the owned entry in `path` from exactly `from` to `to`; an owned entry
/// running anything else, and every other entry, is left alone. `Ok(true)` when it was changed; a
/// missing file changes nothing.
pub fn repoint_memory_mcp(path: &Path, from: &Path, to: &Path) -> Result<bool, McpRefusal> {
    let from = from.to_string_lossy();
    let to = to.to_string_lossy();

    change_file(path, |target| {
        let found = find_server(path, target, &[], MEMORY_MCP_SERVER, MEMORY_BINARY);
        let runs_from = found.is_some_and(|server| {
            owned(path, &server) && server.definition.get("command").and_then(Value::as_str) == Some(&from)
        });

        if !runs_from {
            return Change::Keep(false);
        }

        let mut document = target.document();
        set_command(&mut document, &to);

        Change::Write(document, true)
    })
}

/// Whether any server in `path` has `command` as its `command` (whatever its name and arguments;
/// ASCII case and `/` versus `\` are ignored, so a doubtful match counts). A missing file names
/// nothing; `Err` when the file cannot be read as an MCP file.
pub fn names_command(path: &Path, command: &Path) -> Result<bool, McpRefusal> {
    let normalize = |text: &str| text.replace('\\', "/").to_ascii_lowercase();
    let command = normalize(&command.to_string_lossy());

    let document = load(path)?.document();
    let Some(servers) = document.get(SERVERS).and_then(Value::as_object) else {
        return Ok(false);
    };

    let named = servers
        .values()
        .filter_map(|server| server.get("command").and_then(Value::as_str))
        .any(|found| normalize(found) == command);

    Ok(named)
}

fn when_missing(target: &Target, command: &str, recorded: bool) -> Change<MemoryRegistration> {
    if recorded {
        return Change::Keep(MemoryRegistration::SkippedRemoved);
    }

    let mut document = target.document();
    let servers = document.entry(SERVERS).or_insert_with(|| Value::Object(Map::new()));

    // `load` refuses any other `mcpServers`, so this is always an object.
    if let Value::Object(servers) = servers {
        servers.insert(
            MEMORY_MCP_SERVER.to_owned(),
            json!({ "command": command, "args": ["mcp"] }),
        );
    }

    Change::Write(document, MemoryRegistration::Added)
}

/// Whether the app may change `server` (see the module docs).
fn owned(path: &Path, server: &FoundServer) -> bool {
    server.file == path && server.name == MEMORY_MCP_SERVER && runs_mcp(&server.definition, MEMORY_BINARY)
}

/// Sets the owned entry's `command` in place, keeping its position and its other fields.
fn set_command(document: &mut Map<String, Value>, command: &str) {
    let entry = document
        .get_mut(SERVERS)
        .and_then(Value::as_object_mut)
        .and_then(|servers| servers.get_mut(MEMORY_MCP_SERVER))
        .and_then(Value::as_object_mut);

    if let Some(entry) = entry {
        entry.insert("command".to_owned(), Value::String(command.to_owned()));
    }
}
