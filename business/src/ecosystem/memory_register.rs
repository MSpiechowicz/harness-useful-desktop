//! The installed `harness-memory` MCP server in a harness's MCP file (`register_memory_mcp`,
//! `unregister_memory_mcp`, and `repoint_memory_mcp` for the old copy, see
//! [`super::memory_migrate`]).
//!
//! - Found: like the dashboard's server (see [`super::mcp_register`]): a server named
//!   [`MEMORY_MCP_SERVER`], or any server running `harness-memory` (any directory, `.exe` or not)
//!   with the single argument `mcp`, in the target file or one of the `also_read` files (on
//!   OpenCode 2 under `mcp.servers` or the legacy `mcp`), switched off by the target file's
//!   `disabledServers` or by the server itself.
//! - Owned: an entry in the target file (never an `also_read` one) named exactly
//!   [`MEMORY_MCP_SERVER`] that runs `harness-memory mcp`. Only owned entries are ever changed:
//!   the program they run follows the resolved server (`command`, or the first item of a `command`
//!   array), and turning project memory off removes them. Every other field (`enabled`, `env`, ...)
//!   and the key order are kept. On OpenCode 2 a legacy `mcp.harness-memory` beside an owned
//!   `mcp.servers.harness-memory` is removed with it only when it runs `harness-memory mcp` too;
//!   any other one is kept as it is (and logged, since OpenCode then loads it).
//! - Added: the file's [`Shape::entry`] for the absolute path (`{"command": "<path>", "args":
//!   ["mcp"]}`, OpenCode 1's `{"type": "local", "command": ["<path>", "mcp"], "enabled": true}`, or
//!   OpenCode 2's `{"type": "local", "command": ["<path>", "mcp"]}` under `mcp.servers`), only
//!   when nothing is found and the app has not registered the server in that file before (a
//!   user who removed it keeps it removed).
//!
//! Writes follow [`super::mcp_merge`]'s rules: the same refusals, a JSONC file with comments never
//! written, a new file `0600`, an atomic write keeping permissions, one re-read when the file
//! changed meanwhile. File contents are never logged or returned, except the old and new program of
//! an owned entry that is re-pointed.

use std::path::Path;

use serde_json::{Map, Value};

use super::catalog::{MEMORY_BINARY, MEMORY_MCP_SERVER};
use super::mcp_merge::{Blocked, Change, Shape, Target, change_file, load, remove_server, server_mut, servers};
use super::mcp_register::{FoundServer, add_server, find_server, runs_mcp, server_program};
use super::setup::McpFile;
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
    /// The file has comments (JSONC) a rewrite would lose; nothing was written.
    ManualEdit,
    /// The file cannot be changed safely; nothing was written.
    Refused(McpRefusal),
}

impl MemoryRegistration {
    /// Whether the server is in the file after this registration (enabled or not).
    pub fn is_registered(self) -> bool {
        matches!(self, Self::Added | Self::Updated | Self::Present | Self::Disabled)
    }
}

/// Registers the memory server in `file` as `command mcp` (see the module docs). `recorded` tells
/// that the app registered it in that file before.
pub fn register_memory_mcp(file: &McpFile, command: &Path, recorded: bool) -> MemoryRegistration {
    let command = command.to_string_lossy();
    let changed = change_file(&file.path, file.shape, |target| {
        memory_change(file, target, &command, recorded)
    });

    memory_outcome(changed)
}

/// What registering the memory server as `command mcp` does with `target` (see the module docs).
pub(crate) fn memory_change(
    file: &McpFile,
    target: &Target,
    command: &str,
    recorded: bool,
) -> Change<MemoryRegistration> {
    let Some(server) = find_server(file, target, MEMORY_MCP_SERVER, MEMORY_BINARY) else {
        return when_missing(target, file.shape, command, recorded);
    };

    let previous = server_program(&server.definition);
    let stale = owned(&file.path, &server) && previous != Some(command);

    if stale {
        tracing::info!(
            file = %file.path.display(),
            from = previous.unwrap_or_default(),
            to = %command,
            "project memory: the app's entry now runs another harness-memory"
        );

        let mut document = target.document();
        set_program(&mut document, file.shape, command);
        return Change::Write(document, MemoryRegistration::Updated);
    }

    if server.disabled {
        Change::Keep(MemoryRegistration::Disabled)
    } else {
        Change::Keep(MemoryRegistration::Present)
    }
}

/// The registration a memory change ends in.
pub(crate) fn memory_outcome(changed: Result<MemoryRegistration, Blocked>) -> MemoryRegistration {
    match changed {
        Ok(registration) => registration,
        Err(Blocked::Refused(reason)) => MemoryRegistration::Refused(reason),
        Err(Blocked::ManualEdit) => MemoryRegistration::ManualEdit,
    }
}

/// Removes the owned entry from `file`, leaving every other server and the
/// `disabledServers`/`enabledServers` lists as they are. `Ok(true)` when it was removed.
pub fn unregister_memory_mcp(file: &McpFile) -> Result<bool, Blocked> {
    let target_only = target_only(file);

    change_file(&file.path, file.shape, |target| memory_removal(&target_only, target))
}

/// `file` without the files it also reads: only the target file is changed when the entry is
/// removed, so the others are not looked at.
pub(crate) fn target_only(file: &McpFile) -> McpFile {
    McpFile {
        also_read: Vec::new(),
        ..file.clone()
    }
}

/// What removing the owned entry does with `target`; `file` is the [`target_only`] file.
pub(crate) fn memory_removal(file: &McpFile, target: &Target) -> Change<bool> {
    let found = find_server(file, target, MEMORY_MCP_SERVER, MEMORY_BINARY);

    if !found.is_some_and(|server| owned(&file.path, &server)) {
        return Change::Keep(false);
    }

    let mut document = target.document();
    remove_server(&mut document, file.shape, MEMORY_MCP_SERVER, |entry| {
        runs_mcp(entry, MEMORY_BINARY)
    });

    let still_seen = servers(&document, file.shape).is_some_and(|left| left.contains_key(MEMORY_MCP_SERVER));
    if still_seen {
        tracing::warn!(
            file = %file.path.display(),
            "project memory: the app's entry was removed, but a harness-memory server the app does not own stays and is still loaded"
        );
    }

    Change::Write(document, true)
}

/// Changes the `command` of the owned entry in the `mcpServers` file `path` from exactly `from` to
/// `to`; an owned entry running anything else, and every other entry, is left alone. `Ok(true)` when
/// it was changed; a missing file changes nothing. (Only `mcpServers` files ever named the old copy.)
pub fn repoint_memory_mcp(path: &Path, from: &Path, to: &Path) -> Result<bool, McpRefusal> {
    let from = from.to_string_lossy();
    let to = to.to_string_lossy();
    let file = mcp_servers_file(path);

    let changed = change_file(path, file.shape, |target| {
        let found = find_server(&file, target, MEMORY_MCP_SERVER, MEMORY_BINARY);
        let runs_from =
            found.is_some_and(|server| owned(path, &server) && server_program(&server.definition) == Some(&from));

        if !runs_from {
            return Change::Keep(false);
        }

        let mut document = target.document();
        set_program(&mut document, file.shape, &to);

        Change::Write(document, true)
    });

    changed.map_err(|blocked| match blocked {
        Blocked::Refused(reason) => reason,
        // A `mcpServers` file is strict JSON: comments make it unparsable.
        Blocked::ManualEdit => McpRefusal::Unparsable,
    })
}

/// Whether any server in the `mcpServers` file `path` has `command` as its `command` (whatever its
/// name and arguments; ASCII case and `/` versus `\` are ignored, so a doubtful match counts). A
/// missing file names nothing; `Err` when the file cannot be read as an MCP file.
pub fn names_command(path: &Path, command: &Path) -> Result<bool, McpRefusal> {
    let normalize = |text: &str| text.replace('\\', "/").to_ascii_lowercase();
    let command = normalize(&command.to_string_lossy());

    let shape = Shape::McpServers;
    let document = load(path, shape)?.document();
    let Some(servers) = servers(&document, shape) else {
        return Ok(false);
    };

    let named = servers
        .values()
        .filter_map(server_program)
        .any(|found| normalize(found) == command);

    Ok(named)
}

/// `path` as a `mcpServers` file with nothing else to read.
fn mcp_servers_file(path: &Path) -> McpFile {
    McpFile {
        path: path.to_path_buf(),
        also_read: Vec::new(),
        shape: Shape::McpServers,
    }
}

fn when_missing(target: &Target, shape: Shape, command: &str, recorded: bool) -> Change<MemoryRegistration> {
    if recorded {
        return Change::Keep(MemoryRegistration::SkippedRemoved);
    }

    let mut document = target.document();
    add_server(&mut document, shape, MEMORY_MCP_SERVER, shape.entry(command));

    Change::Write(document, MemoryRegistration::Added)
}

/// Whether the app may change `server` (see the module docs).
fn owned(path: &Path, server: &FoundServer) -> bool {
    server.file == path && server.name == MEMORY_MCP_SERVER && runs_mcp(&server.definition, MEMORY_BINARY)
}

/// Sets the program the owned entry runs (its `command`, or the first item of its `command` array)
/// in place, keeping its position and its other fields.
fn set_program(document: &mut Map<String, Value>, shape: Shape, program: &str) {
    let entry = server_mut(document, shape, MEMORY_MCP_SERVER).and_then(Value::as_object_mut);

    let Some(entry) = entry else {
        return;
    };

    let program = Value::String(program.to_owned());

    match entry.get_mut("command") {
        Some(Value::Array(parts)) if !parts.is_empty() => parts[0] = program,
        _ => {
            entry.insert("command".to_owned(), program);
        }
    }
}
