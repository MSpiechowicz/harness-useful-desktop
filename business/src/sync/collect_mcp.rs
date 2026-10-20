//! Export reader for MCP servers: the user MCP file of [`mcp_target_paths`]. See
//! [`super::collect`].
//!
//! Only an allowlist of fields is shared: the transport, the command and arguments (stdio) or the
//! URL (http/sse), and the key names of `env` and `headers`. Their values, `auth`/`oauth`, `cwd` and
//! any other field stay here (noted on the item). A server whose command, arguments or URL holds a
//! secret, credentials, a local path or an env reference the import would refuse is left out whole.
//! The servers Recommended setup and the app install themselves ([`is_reserved_server`]: the
//! dashboard's, the memory server and the skills' server) are skipped without a row, and the import
//! refuses those names.

use std::path::Path;

use serde_json::{Map, Value};

use super::collect::{CollectedItem, Entry, Found, excluded_reason, item_id, preview_row, safe_label};
use super::schema::{McpServer, Transport};
use super::secrets::{args_exportable, exportable};
use super::validate;
use super::validate_mcp::is_reserved_server;
use crate::ecosystem::mcp_target_paths;
use crate::env::Env;
use crate::harness::Harness;
use crate::model::{SyncExcludedReason, SyncExportItem, SyncItemKind, SyncNote};
use crate::pi_family::{Document, read_json, tilde};

/// The shared form of one server and the notes on what stays here, or why it is left out.
pub(super) type SharedServer = fn(&str, &Value) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason>;

/// Fields that are shared (or deliberately dropped without a note).
const KNOWN_FIELDS: &[&str] = &[
    "type", "command", "args", "url", "env", "headers", "enabled", "auth", "oauth", "cwd",
];

/// The servers of `harness`'s user MCP file; nothing when it has none, the file is missing, or it
/// has no `mcpServers`. A file that cannot be read or parsed, or whose `mcpServers` is not an
/// object, is listed as excluded (`invalid`, labelled with its home-relative path).
pub(super) fn mcp_servers(env: &Env, harness: &dyn Harness, found: &mut Found) {
    let Some((path, _)) = mcp_target_paths(env, harness) else {
        return;
    };

    let document = read_json(&path);
    collect_servers(env, harness, found, &path, &document, "mcpServers", shared_server);
}

/// The servers under `key` of a harness MCP `document` read from `path`, each in its shared form
/// from `shared` or excluded with its reason; nothing when the file is missing or has no `key`.
/// A file that cannot be read or parsed, or whose `key` is not an object, is listed as excluded
/// (`invalid`, labelled with its home-relative path). Reserved servers are skipped without a row.
pub(super) fn collect_servers(
    env: &Env,
    harness: &dyn Harness,
    found: &mut Found,
    path: &Path,
    document: &Document,
    key: &str,
    shared: SharedServer,
) {
    let kind = SyncItemKind::McpServer;
    let servers = match document {
        Document::Missing => return,
        Document::Invalid(_) => None,
        Document::Parsed(value) => match value.get(key) {
            None => return,
            Some(servers) => servers.as_object(),
        },
    };

    let Some(servers) = servers else {
        let label = safe_label(Some(&tilde(env, path)));
        found.exclude(harness.id(), kind, label, SyncExcludedReason::Invalid);
        return;
    };

    for (name, server) in servers {
        if is_reserved_server(name) {
            continue;
        }

        match shared(name, server) {
            Ok((server, notes)) => found.items.push(CollectedItem {
                preview: preview(item_id(harness.id(), kind, name), &server, notes),
                entry: Entry::McpServer(server),
            }),
            Err(reason) => found.exclude(harness.id(), kind, safe_label(Some(name)), reason),
        }
    }
}

pub(super) fn preview(id: String, server: &McpServer, notes: Vec<SyncNote>) -> SyncExportItem {
    SyncExportItem {
        id,
        detail: Some(server.detail()),
        runs_program: server.transport == Transport::Stdio,
        env_keys: server.env_keys.clone(),
        header_keys: server.header_keys.clone(),
        notes,
        ..preview_row(SyncItemKind::McpServer, server.name.clone())
    }
}

/// The shared form of one server and the notes on what stays here, or why it is left out.
fn shared_server(name: &str, server: &Value) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason> {
    validate::mcp_name(name).map_err(excluded_reason)?;

    let fields = server.as_object().ok_or(SyncExcludedReason::Invalid)?;
    let transport = transport(fields).ok_or(SyncExcludedReason::Unsupported)?;

    let (command, args, url) = match transport {
        Transport::Stdio => (Some(text(fields, "command")?), list(fields, "args")?, None),
        Transport::Http | Transport::Sse => (None, Vec::new(), Some(text(fields, "url")?)),
    };

    let server = checked_server(name, fields, transport, command, args, url, "env")?;
    let notes = notes(fields, &server);
    validate::mcp_server(&server).map_err(excluded_reason)?;

    Ok((server, notes))
}

/// `type` when present (`stdio`, `http`/`streamable-http`, `sse`), else stdio for a `command` and
/// HTTP for a `url`; `None` when unknown.
fn transport(fields: &Map<String, Value>) -> Option<Transport> {
    match fields.get("type") {
        Some(Value::String(kind)) => match kind.as_str() {
            "stdio" => Some(Transport::Stdio),
            "http" | "streamable-http" => Some(Transport::Http),
            "sse" => Some(Transport::Sse),
            _ => None,
        },
        Some(_) => None,
        None if fields.contains_key("command") => Some(Transport::Stdio),
        None if fields.contains_key("url") => Some(Transport::Http),
        None => None,
    }
}

/// A required string field.
pub(super) fn text(fields: &Map<String, Value>, key: &str) -> Result<String, SyncExcludedReason> {
    match fields.get(key) {
        Some(Value::String(value)) => Ok(value.clone()),
        _ => Err(SyncExcludedReason::Invalid),
    }
}

/// An optional list of strings.
pub(super) fn list(fields: &Map<String, Value>, key: &str) -> Result<Vec<String>, SyncExcludedReason> {
    let Some(value) = fields.get(key) else {
        return Ok(Vec::new());
    };

    let items = value.as_array().ok_or(SyncExcludedReason::Invalid)?;
    items
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or(SyncExcludedReason::Invalid))
        .collect()
}

/// The key names of an optional object (`env`, `headers`), never its values. Key names that look
/// like secrets leave the server out.
pub(super) fn keys(fields: &Map<String, Value>, key: &str) -> Result<Vec<String>, SyncExcludedReason> {
    let Some(value) = fields.get(key) else {
        return Ok(Vec::new());
    };

    let object = value.as_object().ok_or(SyncExcludedReason::Invalid)?;
    if !object.keys().all(|name| exportable(name)) {
        return Err(SyncExcludedReason::ContainsSecret);
    }

    Ok(object.keys().cloned().collect())
}

/// Why the command, arguments or URL may not leave this machine: credentials, then secrets, then a
/// local path (checked before the import validators so the reason names the actual problem).
pub(super) fn endpoint_problem(
    command: Option<&str>,
    args: &[String],
    url: Option<&str>,
) -> Result<(), SyncExcludedReason> {
    let values = || command.into_iter().chain(url);

    if values().any(validate::has_credentials) {
        return Err(SyncExcludedReason::Credentials);
    }

    if !values().all(exportable) || !args_exportable(args) {
        return Err(SyncExcludedReason::ContainsSecret);
    }

    if values().any(validate::is_local_path) || args.iter().any(|arg| validate::is_local_path(arg)) {
        return Err(SyncExcludedReason::LocalPath);
    }

    Ok(())
}

/// The server once its command, arguments and URL may leave this machine ([`endpoint_problem`]),
/// with the key names of `env_field` and `headers`.
pub(super) fn checked_server(
    name: &str,
    fields: &Map<String, Value>,
    transport: Transport,
    command: Option<String>,
    args: Vec<String>,
    url: Option<String>,
    env_field: &str,
) -> Result<McpServer, SyncExcludedReason> {
    endpoint_problem(command.as_deref(), &args, url.as_deref())?;

    let env_keys = keys(fields, env_field)?;
    let header_keys = keys(fields, "headers")?;

    Ok(McpServer {
        name: name.to_owned(),
        transport,
        command,
        args,
        url,
        env_keys,
        header_keys,
    })
}

/// What is not shared: env and header values, auth, `cwd`, and fields outside the allowlist (also
/// a `url` on a stdio server, or a `command`/`args` on an http/sse server).
fn notes(fields: &Map<String, Value>, server: &McpServer) -> Vec<SyncNote> {
    let unused: &[&str] = match server.transport {
        Transport::Stdio => &["url"],
        Transport::Http | Transport::Sse => &["command", "args"],
    };
    let noted: &[(&[&str], SyncNote)] = &[
        (&["auth", "oauth"], SyncNote::AuthNotShared),
        (&["cwd"], SyncNote::CwdNotShared),
    ];

    not_shared(fields, server, unused, KNOWN_FIELDS, noted)
}

/// The notes of a server read from `fields`: env and header values when it has their keys, each
/// `noted` note whose fields are present, then other fields (an `unused` endpoint field or one
/// outside `known`).
pub(super) fn not_shared(
    fields: &Map<String, Value>,
    server: &McpServer,
    unused: &[&str],
    known: &[&str],
    noted: &[(&[&str], SyncNote)],
) -> Vec<SyncNote> {
    let present = |names: &[&str]| names.iter().any(|name| fields.contains_key(*name));
    let other = present(unused) || fields.keys().any(|key| !known.contains(&key.as_str()));

    let values = [
        (!server.env_keys.is_empty(), SyncNote::EnvValuesNotShared),
        (!server.header_keys.is_empty(), SyncNote::HeaderValuesNotShared),
    ];
    let noted = noted.iter().map(|(names, note)| (present(names), *note));

    values
        .into_iter()
        .chain(noted)
        .chain([(other, SyncNote::OtherFieldsNotShared)])
        .filter_map(|(applies, note)| applies.then_some(note))
        .collect()
}
