//! Adding server entries to a harness's user MCP file (`merge_servers`), reading one entry back
//! for an in-process comparison (`existing_server`), and the read-decide-write cycle the merge
//! and the companions' registrations use (`change_file`).
//!
//! A file has a [`Shape`]: the servers sit under `mcpServers` (a `mcp.json`, strict JSON) or under
//! `mcp` (an OpenCode config file, JSONC). `merge_servers` and `existing_server` work on
//! `mcpServers` files only.
//!
//! - Merge: inserts each entry into `mcpServers`, as given (an `"enabled": false` included), after
//!   the servers already there, keeping every other key, server and value in order. A missing file
//!   is created (mode `0600`, parent directories too); a blank file counts as `{}`.
//! - An existing name is never overwritten: it is reported as `AlreadyPresent` when its definition
//!   is identical, else as `Conflict`, and left as it was. Nothing is written when no entry is added.
//! - Refused, nothing written: invalid JSON, a root or servers object that is not an object, a
//!   symlink, anything but a regular file, more than 2 MiB, or a file that cannot be read.
//! - JSONC: a file of a JSONC shape is read as JSONC. One with comments is never written
//!   ([`Blocked::ManualEdit`]): a rewrite would lose them. One with only trailing commas is
//!   rewritten as plain JSON. A new file of the OpenCode shape starts with OpenCode's `$schema`.
//! - The write is atomic (temporary sibling, flushed, renamed over the file) and keeps the file's
//!   permissions. A file that changed meanwhile is read again once, then refused; a write that
//!   fails (or a second change) is refused as `WriteFailed`, the file left as it was.
//!
//! Callers validate the entries; this module only places them. File contents are never logged or
//! returned, except by [`existing_server`] (see its docs).

use std::fs::{self, Metadata, Permissions};
use std::path::Path;
use std::time::SystemTime;

use serde_json::{Map, Value, json};

use crate::env::Env;
use crate::error::BusinessError;
use crate::fs_atomic::{temp_sibling, write_synced};
use crate::model::McpRefusal;
use crate::pi_family::jsonc;
use crate::security::SafeReader;

/// Largest MCP file read or changed.
const MAX_MCP_BYTES: u64 = SafeReader::DEFAULT_MAX_BYTES;

/// The `$schema` OpenCode writes into its config files.
const OPENCODE_SCHEMA: &str = "https://opencode.ai/config.json";

/// How an MCP file holds its servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `{"mcpServers": {"<name>": {"command": "<binary>", "args": ["mcp"]}}}`, strict JSON (OMP's and
    /// PI's `mcp.json`).
    McpServers,
    /// `{"mcp": {"<name>": {"type": "local", "command": ["<binary>", "mcp"], "enabled": true}}}`,
    /// JSONC (OpenCode's config file).
    OpenCode,
}

impl Shape {
    /// The key holding the servers.
    pub fn root(self) -> &'static str {
        match self {
            Self::McpServers => "mcpServers",
            Self::OpenCode => "mcp",
        }
    }

    /// The entry that runs `<command> mcp`.
    pub fn entry(self, command: &str) -> Value {
        match self {
            Self::McpServers => json!({ "command": command, "args": ["mcp"] }),
            Self::OpenCode => json!({ "type": "local", "command": [command, "mcp"], "enabled": true }),
        }
    }

    /// Whether the file is JSONC (comments, trailing commas).
    fn jsonc(self) -> bool {
        self == Self::OpenCode
    }

    /// The document a missing file starts as.
    fn new_document(self) -> Map<String, Value> {
        let mut document = Map::new();

        if self == Self::OpenCode {
            document.insert("$schema".to_owned(), Value::String(OPENCODE_SCHEMA.to_owned()));
        }

        document
    }
}

/// Why [`change_file`] left a file alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// The file cannot be changed safely.
    Refused(McpRefusal),
    /// The file has comments (JSONC) a rewrite would lose; the user edits it.
    ManualEdit,
}

/// What [`merge_servers`] did with a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The file was read; one result per entry, in the order given. It was written only when at
    /// least one entry is [`MergeResult::Added`].
    Merged(Vec<MergedServer>),
    /// The file is left alone as a whole; no entry was added.
    Refused(McpRefusal),
}

/// What happened to one entry of a [`merge_servers`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedServer {
    pub name: String,
    pub result: MergeResult,
}

/// Result for one entry of a merge that was not refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeResult {
    /// Written to the file.
    Added,
    /// A server of that name with an identical definition was already there.
    AlreadyPresent,
    /// A server of that name with another definition is there; it was left untouched.
    Conflict,
}

/// Adds `entries` (name, definition) to `mcpServers` in `path` without overwriting any existing
/// name (see the module docs). A name given twice is compared with its first occurrence.
///
/// `env` is unused for now; it keeps the signature in line with the other MCP file functions.
pub fn merge_servers(_env: &Env, path: &Path, entries: &[(String, Value)]) -> MergeOutcome {
    let changed = change_file(path, Shape::McpServers, |target| {
        let mut document = target.document();
        let results = insert(&mut document, entries);
        let added = results.iter().any(|server| server.result == MergeResult::Added);

        if added {
            Change::Write(document, results)
        } else {
            Change::Keep(results)
        }
    });

    match changed {
        Ok(results) => MergeOutcome::Merged(results),
        Err(Blocked::Refused(reason)) => MergeOutcome::Refused(reason),
        // A `mcpServers` file is strict JSON, so comments make it unparsable rather than this.
        Err(Blocked::ManualEdit) => MergeOutcome::Refused(McpRefusal::Unparsable),
    }
}

/// The definition of the server `name` in `path`'s `mcpServers`, or `None` when the file or the
/// server does not exist. Refused like a merge (see the module docs).
///
/// The definition is returned as written, secrets (env, headers, ...) included: it is for comparing
/// in process only, and must never be logged, returned over IPC or shown.
pub fn existing_server(_env: &Env, path: &Path, name: &str) -> Result<Option<Value>, McpRefusal> {
    let shape = Shape::McpServers;

    match load(path, shape)? {
        Target::Absent { .. } => Ok(None),
        Target::Present { document, .. } => {
            Ok(servers(&document, shape).and_then(|servers| servers.get(name)).cloned())
        }
    }
}

/// The servers object of `document`, when there is one.
pub(crate) fn servers(document: &Map<String, Value>, shape: Shape) -> Option<&Map<String, Value>> {
    document.get(shape.root()).and_then(Value::as_object)
}

/// The servers object of `document`, created when missing (`None` when the key holds something
/// else, which [`load`] refuses).
pub(crate) fn servers_mut(document: &mut Map<String, Value>, shape: Shape) -> Option<&mut Map<String, Value>> {
    document
        .entry(shape.root())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
}

/// What a [`change_file`] decision does with the file it was shown.
pub(crate) enum Change<R> {
    /// Leaves the file as it is and returns the value.
    Keep(R),
    /// Writes the document over the file and returns the value once written.
    Write(Map<String, Value>, R),
}

/// Reads `path` as a `shape` file and lets `decide` keep it or replace its document, under the
/// module's rules: the same refusals (returned as `Err`, nothing written), a JSONC file with
/// comments never written, an atomic write keeping the permissions (a new file is `0600`), and one
/// more read and decision when the file changed meanwhile.
pub(crate) fn change_file<R>(
    path: &Path,
    shape: Shape,
    mut decide: impl FnMut(&Target) -> Change<R>,
) -> Result<R, Blocked> {
    for _attempt in 0..2 {
        let target = load(path, shape).map_err(Blocked::Refused)?;

        let (document, value) = match decide(&target) {
            Change::Keep(value) => return Ok(value),
            Change::Write(document, value) => (document, value),
        };

        let original = match target {
            Target::Absent { .. } => None,
            Target::Present { rewritable: false, .. } => return Err(Blocked::ManualEdit),
            Target::Present { stamp, permissions, .. } => Some((stamp, permissions)),
        };

        match write(path, document, original.as_ref()) {
            Ok(()) => return Ok(value),
            Err(WriteError::Changed) => {}
            Err(WriteError::Failed) => return Err(Blocked::Refused(McpRefusal::WriteFailed)),
        }
    }

    // Changed again after the second read.
    Err(Blocked::Refused(McpRefusal::WriteFailed))
}

/// Inserts the entries whose names are free and reports each one.
fn insert(document: &mut Map<String, Value>, entries: &[(String, Value)]) -> Vec<MergedServer> {
    let Some(servers) = servers_mut(document, Shape::McpServers) else {
        return Vec::new();
    };

    entries
        .iter()
        .map(|(name, definition)| {
            let result = match servers.get(name) {
                None => {
                    servers.insert(name.clone(), definition.clone());
                    MergeResult::Added
                }
                Some(existing) if existing == definition => MergeResult::AlreadyPresent,
                Some(_) => MergeResult::Conflict,
            };

            MergedServer {
                name: name.clone(),
                result,
            }
        })
        .collect()
}

/// The target file as read.
pub(crate) enum Target {
    /// No file; `document` is what a new one starts as.
    Absent { document: Map<String, Value> },
    Present {
        document: Map<String, Value>,
        stamp: Stamp,
        permissions: Permissions,
        /// False for a JSONC file with comments.
        rewritable: bool,
    },
}

impl Target {
    /// A copy of the document read (or the one a missing file starts as).
    pub(crate) fn document(&self) -> Map<String, Value> {
        match self {
            Target::Absent { document } | Target::Present { document, .. } => document.clone(),
        }
    }

    /// Whether the file can be rewritten (a missing one can be created).
    pub(crate) fn rewritable(&self) -> bool {
        match self {
            Target::Absent { .. } => true,
            Target::Present { rewritable, .. } => *rewritable,
        }
    }
}

/// What tells that a file changed since it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
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

/// Reads `path` as a `shape` MCP file, or says why it is refused.
pub(crate) fn load(path: &Path, shape: Shape) -> Result<Target, McpRefusal> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Target::Absent {
                document: shape.new_document(),
            });
        }
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

    let (document, rewritable) = parse(&text, shape)?;

    Ok(Target::Present {
        document,
        stamp: Stamp::of(&metadata),
        permissions: metadata.permissions(),
        rewritable,
    })
}

/// The document of `text`, and whether it may be rewritten (false for JSONC with comments).
fn parse(text: &str, shape: Shape) -> Result<(Map<String, Value>, bool), McpRefusal> {
    if text.trim().is_empty() {
        return Ok((Map::new(), true));
    }

    let (value, rewritable) = if shape.jsonc() {
        let value = jsonc::parse(text).map_err(|_| McpRefusal::Unparsable)?;
        (value, !jsonc::has_comments(text))
    } else {
        let value = serde_json::from_str(text).map_err(|_| McpRefusal::Unparsable)?;
        (value, true)
    };

    let document = match value {
        Value::Object(document) => document,
        // Only comments: OpenCode reads it as an empty config.
        Value::Null if shape.jsonc() => Map::new(),
        _ => return Err(McpRefusal::NotAnObject),
    };

    match document.get(shape.root()) {
        None | Some(Value::Object(_)) => Ok((document, rewritable)),
        Some(_) => Err(McpRefusal::ServersNotAnObject),
    }
}

/// Why a write did not happen.
enum WriteError {
    /// The file changed after it was read.
    Changed,
    /// Creating, writing or renaming failed.
    Failed,
}

/// Writes `document` atomically over `path`; `original` is what was read (`None`: no file).
fn write(path: &Path, document: Map<String, Value>, original: Option<&(Stamp, Permissions)>) -> Result<(), WriteError> {
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(document)).map_err(|_| WriteError::Failed)?;
    bytes.push(b'\n');

    if original.is_none()
        && let Some(parent) = path.parent()
    {
        fs::create_dir_all(parent).map_err(|_| WriteError::Failed)?;
    }

    let temp = temp_sibling(path);
    let written = replace(path, &temp, &bytes, original);

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
