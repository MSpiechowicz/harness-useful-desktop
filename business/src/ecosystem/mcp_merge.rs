//! Adding server entries to a harness's user MCP file (`merge_servers`) and reading one entry back
//! for an in-process comparison (`existing_server`).
//!
//! - Merge: inserts each entry into `mcpServers`, as given (an `"enabled": false` included), after
//!   the servers already there, keeping every other key, server and value in order. A missing file
//!   is created (mode `0600`, parent directories too); a blank file counts as `{}`.
//! - An existing name is never overwritten: it is reported as `AlreadyPresent` when its definition
//!   is identical, else as `Conflict`, and left as it was. Nothing is written when no entry is added.
//! - Refused, nothing written: invalid JSON, a root or `mcpServers` that is not an object, a symlink,
//!   anything but a regular file, more than 2 MiB, or a file that cannot be read.
//! - The write is atomic (temporary sibling, flushed, renamed over the file) and keeps the file's
//!   permissions. A file that changed meanwhile is read again once, then refused; a write that
//!   fails (or a second change) is refused as `WriteFailed`, the file left as it was.
//!
//! Callers validate the entries; this module only places them. File contents are never logged or
//! returned, except by [`existing_server`] (see its docs).

use std::convert::Infallible;
use std::fs::{self, Metadata, Permissions};
use std::path::Path;
use std::time::SystemTime;

use serde_json::{Map, Value};

use crate::env::Env;
use crate::error::BusinessError;
use crate::fs_atomic::{temp_sibling, write_synced};
use crate::model::McpRefusal;
use crate::security::SafeReader;

/// Largest MCP file read or changed.
const MAX_MCP_BYTES: u64 = SafeReader::DEFAULT_MAX_BYTES;

/// The key holding the servers.
pub(crate) const SERVERS: &str = "mcpServers";

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
    match merge_servers_unless(path, entries, |_| None::<Infallible>) {
        Ok(outcome) => outcome,
        Err(never) => match never {},
    }
}

/// The definition of the server `name` in `path`'s `mcpServers`, or `None` when the file or the
/// server does not exist. Refused like a merge (see the module docs).
///
/// The definition is returned as written, secrets (env, headers, ...) included: it is for comparing
/// in process only, and must never be logged, returned over IPC or shown.
pub fn existing_server(_env: &Env, path: &Path, name: &str) -> Result<Option<Value>, McpRefusal> {
    match load(path)? {
        Target::Absent => Ok(None),
        Target::Present { document, .. } => Ok(document
            .get(SERVERS)
            .and_then(Value::as_object)
            .and_then(|servers| servers.get(name))
            .cloned()),
    }
}

/// [`merge_servers`], except that each read of the file is first passed to `stop`: when it returns
/// a value, nothing is written and that value is returned as `Err`.
pub(crate) fn merge_servers_unless<T>(
    path: &Path,
    entries: &[(String, Value)],
    mut stop: impl FnMut(&Target) -> Option<T>,
) -> Result<MergeOutcome, T> {
    for _attempt in 0..2 {
        let target = match load(path) {
            Ok(target) => target,
            Err(reason) => return Ok(MergeOutcome::Refused(reason)),
        };

        if let Some(stopped) = stop(&target) {
            return Err(stopped);
        }

        let (mut document, original) = match target {
            Target::Absent => (Map::new(), None),
            Target::Present {
                document,
                stamp,
                permissions,
            } => (document, Some((stamp, permissions))),
        };

        let results = insert(&mut document, entries);
        let added = results.iter().any(|server| server.result == MergeResult::Added);

        if !added {
            return Ok(MergeOutcome::Merged(results));
        }

        match write(path, document, original.as_ref()) {
            Ok(()) => return Ok(MergeOutcome::Merged(results)),
            Err(WriteError::Changed) => {}
            Err(WriteError::Failed) => return Ok(MergeOutcome::Refused(McpRefusal::WriteFailed)),
        }
    }

    // Changed again after the second read.
    Ok(MergeOutcome::Refused(McpRefusal::WriteFailed))
}

/// Inserts the entries whose names are free and reports each one.
fn insert(document: &mut Map<String, Value>, entries: &[(String, Value)]) -> Vec<MergedServer> {
    let servers = document.entry(SERVERS).or_insert_with(|| Value::Object(Map::new()));

    // `load` refuses any other `mcpServers`, so this is always an object.
    let Value::Object(servers) = servers else {
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
    Absent,
    Present {
        document: Map<String, Value>,
        stamp: Stamp,
        permissions: Permissions,
    },
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

/// Reads `path` as an MCP file, or says why it is refused.
pub(crate) fn load(path: &Path) -> Result<Target, McpRefusal> {
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
