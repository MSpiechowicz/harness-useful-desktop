//! Harness Knowledge's `config.toml`: reading it ([`read_config`]), writing it ([`render`],
//! [`write_config`]) and removing it ([`remove_config`]).
//!
//! The file handling (the refusals, the atomic write, the permissions) is
//! [`sidecar_config`](crate::ecosystem::sidecar_config)'s. This file knows the one table Harness
//! Knowledge reads: `[database]` with `url` and `token`, and `[routing]` with
//! `repository_documents` and `global_documents` (`remote`, `local` or `remote+local`). The server
//! refuses any other key, in a table or beside them, so a file with one is reported and never
//! overwritten.

use std::path::Path;

use toml::{Table, Value};

use crate::ecosystem::catalog::KNOWLEDGE_SIDECAR;
use crate::ecosystem::memory_remote::routing::notes_name;
use crate::ecosystem::sidecar_config::{
    ConfigSpec, ConfigStatus, push_string, read_config_file, remove_config_file, string_value, write_config_file,
};
use crate::error::Result;
use crate::model::MemoryNotesRouting;

/// The table of the database settings.
const TABLE: &str = "database";

/// The key of the database URL in the table.
const URL_KEY: &str = "url";

/// The key of the libSQL token in the table.
const TOKEN_KEY: &str = "token";

/// The table of the routing settings.
const ROUTING_TABLE: &str = "routing";

/// The key of the routing of project documents in the routing table.
const REPOSITORY_KEY: &str = "repository_documents";

/// The key of the routing of global documents in the routing table.
const GLOBAL_KEY: &str = "global_documents";

/// The keys Harness Knowledge reads beside the file's tables (the two tables).
const KNOWN_KEYS: [&str; 2] = [TABLE, ROUTING_TABLE];

/// The knowledge server's config file.
const SPEC: ConfigSpec = ConfigSpec {
    sidecar: &KNOWLEDGE_SIDECAR,
    known_keys: &KNOWN_KEYS,
};

/// What the config file holds, and what is wrong with it.
#[derive(Debug, Default)]
pub(super) struct ConfigFile {
    /// `[database] url`, when it is a string.
    pub url: Option<String>,
    /// `[database] token`, when it is a non-empty string.
    pub token: Option<String>,
    /// `[routing] repository_documents` (`remote` when unset).
    pub repository_documents: MemoryNotesRouting,
    /// `[routing] global_documents` (`remote` when unset).
    pub global_documents: MemoryNotesRouting,
    /// Whether the app may change the file, and the first thing wrong with it.
    pub status: ConfigStatus,
}

/// The file at `path`: empty (and fine) when it does not exist. See [`read_config_file`].
pub(super) fn read_config(path: &Path) -> ConfigFile {
    let (mut config, status) = read_config_file(&SPEC, path, parse_config);

    config.status = status;
    config
}

/// The settings in `table`, whose top-level keys are all known.
fn parse_config(table: &Table, status: &mut ConfigStatus) -> ConfigFile {
    let mut config = parse_database(table, status);

    match table.get(ROUTING_TABLE) {
        None => {}
        Some(Value::Table(routing)) => {
            if routing.keys().any(|key| key != REPOSITORY_KEY && key != GLOBAL_KEY) {
                block(
                    status,
                    format!(
                        "the {ROUTING_TABLE} table has keys {} does not know; remove them by hand first",
                        KNOWLEDGE_SIDECAR.product
                    ),
                );
            }

            config.repository_documents = routing_value(routing, REPOSITORY_KEY, status);
            config.global_documents = routing_value(routing, GLOBAL_KEY, status);
        }
        Some(_) => block(status, format!("{ROUTING_TABLE} must be a table")),
    }

    config
}

/// The routing `key` of `table` read as the server reads it (blanks and case ignored), `remote` when
/// unset. A value the server would refuse blocks the file, so it is never overwritten with `remote`.
fn routing_value(table: &Table, key: &str, status: &mut ConfigStatus) -> MemoryNotesRouting {
    let Some(value) = table.get(key) else {
        return MemoryNotesRouting::default();
    };

    let parsed = value.as_str().and_then(parse_routing);

    if parsed.is_none() {
        block(
            status,
            format!("{key} must be \"remote\", \"local\" or \"remote+local\"; fix it by hand first"),
        );
    }

    parsed.unwrap_or_default()
}

/// `remote`, `local` or `remote+local` in any case with blanks around it, as the server parses it.
pub(super) fn parse_routing(raw: &str) -> Option<MemoryNotesRouting> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "remote" => Some(MemoryNotesRouting::Remote),
        "local" => Some(MemoryNotesRouting::Local),
        "remote+local" => Some(MemoryNotesRouting::RemoteLocal),
        _ => None,
    }
}

/// The `[database]` table of `table`.
fn parse_database(table: &Table, status: &mut ConfigStatus) -> ConfigFile {
    let Some(database) = table.get(TABLE) else {
        return ConfigFile::default();
    };

    let Value::Table(database) = database else {
        block(status, format!("{TABLE} must be a table with url and token"));
        return ConfigFile::default();
    };

    let unknown = database.keys().any(|key| key != URL_KEY && key != TOKEN_KEY);

    if unknown {
        block(
            status,
            format!(
                "the {TABLE} table has keys {} does not know; remove them by hand first",
                KNOWLEDGE_SIDECAR.product
            ),
        );
    }

    ConfigFile {
        url: string_value(database, URL_KEY, status),
        token: string_value(database, TOKEN_KEY, status).filter(|token| !token.trim().is_empty()),
        status: ConfigStatus::default(),
        ..ConfigFile::default()
    }
}

/// Notes `reason` as what blocks the app from changing the file (and the server from using it).
fn block(status: &mut ConfigStatus, reason: String) {
    status.refused = true;
    status.note(reason.clone());
    status.blocked.get_or_insert(reason);
}

/// The file's text: a comment, then the `[database]` table with the URL and the token when there
/// is one.
fn render_database(url: &str, token: Option<&str>) -> String {
    let mut text = format!(
        "# {} team database, saved by Harness Desktop. It holds the database credentials:\n\
         # keep it private (chmod 600).\n[{TABLE}]\n",
        KNOWLEDGE_SIDECAR.product
    );

    push_string(&mut text, URL_KEY, url);

    if let Some(token) = token {
        push_string(&mut text, TOKEN_KEY, token);
    }

    text
}

/// Replaces the file at `path` with `text` (see [`write_config_file`]).
pub(super) fn write_config(path: &Path, text: &str) -> Result<()> {
    write_config_file(&KNOWLEDGE_SIDECAR, path, text)
}

/// Removes the file at `path` (see [`remove_config_file`]).
pub(super) fn remove_config(path: &Path) -> Result<()> {
    remove_config_file(&KNOWLEDGE_SIDECAR, path)
}

/// The file's text with the `[routing]` table after the database, written only when a place is
/// not the default (`remote`): a server that predates routing refuses the table, and a file
/// without it means `remote` for both.
pub(super) fn render(url: &str, token: Option<&str>, routes: Routes) -> String {
    let mut text = render_database(url, token);

    if routes.repository != MemoryNotesRouting::Remote || routes.global != MemoryNotesRouting::Remote {
        text.push_str(&format!("\n[{ROUTING_TABLE}]\n"));
        push_string(&mut text, REPOSITORY_KEY, notes_name(routes.repository));
        push_string(&mut text, GLOBAL_KEY, notes_name(routes.global));
    }

    text
}

/// Where each kind of document goes.
#[derive(Debug, Clone, Copy)]
pub(super) struct Routes {
    pub repository: MemoryNotesRouting,
    pub global: MemoryNotesRouting,
}
